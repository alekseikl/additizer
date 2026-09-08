use std::{array, convert::identity};

mod config;
mod link;
mod ui_bridge;

pub use config::PitchConfig;
use link::{AudioEnd, UiEnd, UiEvent, create_link_pair};
pub use ui_bridge::PitchUiBridge;

use crate::{
    synth_engine::{
        SmoothedSampleParams, StereoSample,
        buffer::{Buffer, VoicesLayout, add_buffer_value, new_voices_layout, zero_buffer},
        routing::{
            ControlRouterType, DataType, Input, InputMeta, InputSlots, ModuleId, NUM_CHANNELS,
            ProcessContext, RouterFactory, SamplesOutput, SpectralInputSlot, VoiceEvent,
            VoiceRouter, VoiceTarget,
        },
        smooth::SmoothedSample,
        synth_module::SynthModule,
        types::Sample,
    },
    utils::{C4_PITCH, from_ms, from_st},
};

const MAX_GLIDE: Sample = 5.0;
const GLIDE_TIME_THRESHOLD: Sample = from_ms(1.0);

struct Params {
    keytrack: bool,
    glide_always: bool,
    glide_per_octave: bool,
}

impl Params {
    fn from_config(c: &PitchConfig) -> Self {
        Self {
            keytrack: c.keytrack,
            glide_always: c.glide_always,
            glide_per_octave: c.glide_per_octave,
        }
    }
}

struct ChannelParams {
    pitch_shift: SmoothedSample, // Octaves
    glide: Sample,
    glide_slope: Sample,
}

impl ChannelParams {
    fn from_config(c: &PitchConfig, channel_idx: usize) -> Self {
        Self {
            pitch_shift: c.pitch_shift[channel_idx].into(),
            glide: c.glide[channel_idx],
            glide_slope: c.glide_slope[channel_idx],
        }
    }

    pub fn advance_smoothers(&mut self, smooth_params: &SmoothedSampleParams, samples: usize) {
        self.pitch_shift.advance(smooth_params, samples);
    }
}

struct Glide {
    t: Sample,
    pitch_from: Sample,
    current_pitch: Sample,
}

impl Glide {
    fn new(pitch_from: Sample) -> Self {
        Self {
            pitch_from,
            current_pitch: pitch_from,
            t: 0.0,
        }
    }
}

struct Voice {
    pitch: Sample, // Octave units
    glide: Option<Glide>,
}

impl Default for Voice {
    fn default() -> Self {
        Self {
            pitch: 0.0,
            glide: None,
        }
    }
}

pub struct Inputs {
    pitch_shift: InputSlots,
    glide: InputSlots,
    glide_slope: InputSlots,
}

impl Default for Inputs {
    fn default() -> Self {
        Self {
            pitch_shift: InputSlots::new(Input::PitchShift),
            glide: InputSlots::new(Input::Glide),
            glide_slope: InputSlots::new(Input::GlideSlope),
        }
    }
}

impl Inputs {
    fn from_slots(inputs: &[InputSlots], _spectral_inputs: &[SpectralInputSlot]) -> Self {
        let mut result = Self::default();

        for input in inputs {
            match input.input_type {
                Input::PitchShift => result.pitch_shift = input.clone(),
                Input::Glide => result.glide = input.clone(),
                Input::GlideSlope => result.glide_slope = input.clone(),
                _ => (),
            }
        }

        result
    }

    fn update_amount(&mut self, input_type: Input, src_slot: usize, amount: StereoSample) {
        match input_type {
            Input::PitchShift => self.pitch_shift.update_amount(src_slot, amount),
            Input::Glide => self.glide.update_amount(src_slot, amount),
            Input::GlideSlope => self.glide_slope.update_amount(src_slot, amount),
            _ => (),
        }
    }
}

struct Buffers {
    pitch: Buffer,
}

type Router<'v, 'f, 'c> = VoiceRouter<'v, 'f, 'c, ControlRouterType>;

pub struct Pitch {
    id: ModuleId,
    params: Params,
    channel_params: [ChannelParams; NUM_CHANNELS],
    buffers: Buffers,
    audio_end: AudioEnd,
    ui_end: Option<UiEnd>,
    inputs: Inputs,
    output_slot: usize,
    voices: VoicesLayout<Voice>,
}

impl Pitch {
    pub fn new(id: ModuleId) -> Self {
        Self::from_config(&PitchConfig {
            id,
            ..PitchConfig::default()
        })
    }

    pub fn from_config(config: &PitchConfig) -> Self {
        let (audio_end, ui_end) = create_link_pair();

        Self {
            id: config.id,
            params: Params::from_config(config),
            channel_params: array::from_fn(|channel_idx| {
                ChannelParams::from_config(config, channel_idx)
            }),
            buffers: Buffers {
                pitch: zero_buffer(),
            },
            audio_end,
            ui_end: Some(ui_end),
            inputs: Inputs::default(),
            output_slot: usize::MAX,
            voices: new_voices_layout(),
        }
    }

    pub fn get_config(&self) -> PitchConfig {
        PitchConfig {
            id: self.id,
            keytrack: self.params.keytrack,
            glide_always: self.params.glide_always,
            glide_per_octave: self.params.glide_per_octave,
            pitch_shift: get_smoothed_param!(self, pitch_shift),
            glide: get_stereo_param!(self, glide),
            glide_slope: get_stereo_param!(self, glide_slope),
        }
    }

    set_mono_param!(set_keytrack, keytrack, bool);
    set_mono_param!(set_glide_always, glide_always, bool);
    set_mono_param!(set_glide_per_octave, glide_per_octave, bool);

    set_smoothed_param!(
        set_pitch_shift,
        pitch_shift,
        pitch_shift.clamp(from_st(-60.0), from_st(60.0))
    );
    set_stereo_param!(set_glide, glide, glide.clamp(0.0, MAX_GLIDE));
    set_stereo_param!(set_glide_slope, glide_slope, glide_slope.clamp(-1.0, 1.0));

    fn glide_time(
        channel: &ChannelParams,
        inputs: &Inputs,
        pitch: Sample,
        pitch_from: Sample,
        router: &mut Router<'_, '_, '_>,
        per_octave: bool,
    ) -> Sample {
        let mut glide_time = router
            .scalar(&inputs.glide, channel.glide)
            .clamp(0.0, MAX_GLIDE);

        if per_octave {
            glide_time *= (pitch - pitch_from).abs();
        }

        glide_time
    }

    fn glide_active(glide_time: Sample, t: Sample) -> bool {
        glide_time >= GLIDE_TIME_THRESHOLD && glide_time - t > 0.0
    }

    fn process_glide(
        channel: &ChannelParams,
        inputs: &Inputs,
        buffers: &mut Buffers,
        voice: &mut Voice,
        router: &mut Router<'_, '_, '_>,
        per_octave: bool,
        samples: usize,
    ) {
        const GLIDE_POWER_MAX: Sample = 6.0;
        const POWER_LINEAR_THRESHOLD: Sample = 0.005;

        let pitch = voice.pitch;

        let Some(glide) = voice.glide.as_mut() else {
            return;
        };

        let glide_time =
            Self::glide_time(channel, inputs, pitch, glide.pitch_from, router, per_octave);
        let time_left = glide_time - glide.t;

        if !Self::glide_active(glide_time, glide.t) {
            voice.glide = None;
            return;
        }

        let glide_slope = router
            .scalar(&inputs.glide_slope, channel.glide_slope)
            .clamp(-1.0, 1.0);
        let glide_power = -glide_slope * GLIDE_POWER_MAX;
        let t_step = router.sample_rate().recip();
        let glide_samples = samples.min((time_left * router.sample_rate()) as usize);
        let pitch_buff = &mut buffers.pitch[..glide_samples];

        #[inline(always)]
        fn process(
            buff: &mut [Sample],
            glide: &mut Glide,
            glide_time: Sample,
            pitch: Sample,
            t_step: Sample,
            curve: impl Fn(Sample) -> Sample,
        ) {
            let pitch_diff = pitch - glide.pitch_from;
            let glide_time_recip = glide_time.recip();

            for out_pitch in buff {
                let diff = pitch_diff * (1.0 - curve(glide.t * glide_time_recip));

                glide.current_pitch = pitch - diff;
                *out_pitch -= diff;
                glide.t += t_step;
            }
        }

        if glide_power.abs() < POWER_LINEAR_THRESHOLD {
            process(pitch_buff, glide, glide_time, pitch, t_step, identity);
        } else {
            let denominator_mult = (glide_power.exp() - 1.0).recip();

            process(pitch_buff, glide, glide_time, pitch, t_step, |v| {
                ((v * glide_power).exp() - 1.0) * denominator_mult
            });
        }

        if glide_samples < samples {
            voice.glide = None;
        }
    }

    fn process_voice(
        &mut self,
        target: &VoiceTarget,
        outputs: &mut VoicesLayout<SamplesOutput>,
        rf: &mut RouterFactory<ControlRouterType>,
    ) {
        let channel_idx = target.channel_idx;
        let voice_idx = target.voice_idx;
        let (mut router, mut voice_output) = rf.for_voice(target, outputs);
        let channel = &self.channel_params[channel_idx];
        let voice = &mut self.voices[channel_idx][voice_idx];
        let inputs = &self.inputs;
        let buffers = &mut self.buffers;

        let output = voice_output.output();
        let samples = output.len();

        router.param(
            &inputs.pitch_shift,
            &channel.pitch_shift,
            &mut buffers.pitch,
        );
        add_buffer_value(&mut buffers.pitch[..samples], voice.pitch);

        Self::process_glide(
            channel,
            inputs,
            buffers,
            voice,
            &mut router,
            self.params.glide_per_octave,
            samples,
        );

        output.copy_from_slice(&buffers.pitch[..samples]);

        if router.need_update_ui_mono() {
            self.audio_end.update_pitch(buffers.pitch[samples - 1]);
        }
    }

    fn handle_trigger(
        &mut self,
        channel_idx: usize,
        prev_pitch: Option<Sample>,
        voice_idx: usize,
        pitch: Sample,
    ) {
        let voice = &mut self.voices[channel_idx][voice_idx];

        voice.glide = None;

        if self.params.keytrack {
            if self.params.glide_always
                && let Some(prev_pitch) = prev_pitch
            {
                voice.glide = Some(Glide::new(prev_pitch));
            }
            voice.pitch = pitch;
        } else {
            voice.pitch = C4_PITCH;
        }
    }

    fn handle_update(&mut self, channel_idx: usize, voice_idx: usize, pitch: Sample) {
        let voice = &mut self.voices[channel_idx][voice_idx];

        if self.params.keytrack {
            voice.glide = Some(Glide::new(
                voice
                    .glide
                    .as_ref()
                    .map_or(voice.pitch, |g| g.current_pitch),
            ));
            voice.pitch = pitch;
        } else {
            voice.glide = None;
            voice.pitch = C4_PITCH;
        }
    }
}

impl SynthModule for Pitch {
    fn id(&self) -> ModuleId {
        self.id
    }

    fn inputs(&self) -> &'static [InputMeta] {
        static INPUTS: &[InputMeta] = &[
            InputMeta::control(Input::PitchShift),
            InputMeta::control(Input::Glide),
            InputMeta::control(Input::GlideSlope),
        ];

        INPUTS
    }

    fn output_type(&self) -> DataType {
        DataType::Control
    }

    fn output_slot(&self) -> usize {
        self.output_slot
    }

    fn set_output_slot(&mut self, slot: usize) {
        self.output_slot = slot;
    }

    fn set_input_slots(&mut self, inputs: &[InputSlots], spectral_inputs: &[SpectralInputSlot]) {
        self.inputs = Inputs::from_slots(inputs, spectral_inputs);
    }

    fn update_input_amount(&mut self, input_type: Input, src_slot: usize, amount: StereoSample) {
        self.inputs.update_amount(input_type, src_slot, amount);
    }

    fn process_events(&mut self, events: &[VoiceEvent]) {
        for event in events {
            match event {
                VoiceEvent::Reset {
                    voice_idx,
                    prev_pitch,
                    pitch,
                    ..
                } => {
                    for channel_idx in 0..NUM_CHANNELS {
                        self.handle_trigger(channel_idx, *prev_pitch, *voice_idx, *pitch);
                    }
                }
                VoiceEvent::Update {
                    voice_idx, pitch, ..
                } => {
                    for channel_idx in 0..NUM_CHANNELS {
                        self.handle_update(channel_idx, *voice_idx, *pitch);
                    }
                }
                _ => (),
            }
        }
    }

    fn process_ui_events(&mut self) {
        while let Some(event) = self.audio_end.pop_event() {
            match event {
                UiEvent::InputParam { input, value } => match input {
                    Input::PitchShift => self.set_pitch_shift(value),
                    Input::Glide => self.set_glide(value),
                    Input::GlideSlope => self.set_glide_slope(value),
                    _ => (),
                },
                UiEvent::Keytrack(keytrack) => self.set_keytrack(keytrack),
                UiEvent::GlideAlways(glide_always) => self.set_glide_always(glide_always),
                UiEvent::GlidePerOctave(glide_per_octave) => {
                    self.set_glide_per_octave(glide_per_octave)
                }
            }
        }
    }

    fn process(&mut self, ctx: &mut ProcessContext) {
        ctx.control(self.id, self.output_slot)
            .for_voices(|rf, target, outputs| {
                self.process_voice(target, outputs, rf);
            })
            .for_channels(|rf, channel_idx| {
                self.channel_params[channel_idx]
                    .advance_smoothers(&rf.params().smooth_params, rf.params().samples);
            });
    }
}
