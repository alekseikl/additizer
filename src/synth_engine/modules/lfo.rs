use std::{array, f32};

use itertools::izip;

mod config;
mod link;
mod ui_bridge;

pub use config::{LfoConfig, LfoShape};
use link::{AudioEnd, UiEnd, UiEvent, create_link_pair};
pub use ui_bridge::LfoUiBridge;

use crate::synth_engine::{
    Input, ModuleId, ModuleType, Sample, StereoSample, SynthModule,
    buffer::{Buffer, new_voices_layout, zero_buffer},
    phase::Phase,
    routing::{DataType, MAX_VOICES, NUM_CHANNELS, Router, VoiceEvent},
    smooth::Smoother,
    synth_module::{ModInput, ProcessParams, VoiceRouter, VoiceRouterFactory},
    types::ScalarOutput,
};

struct ChannelParams {
    frequency: Sample,
    phase_shift: Sample,
    skew: Sample,
    smooth_time: Sample,
}

impl ChannelParams {
    fn from_config(c: &config::LfoConfig, channel_idx: usize) -> Self {
        Self {
            frequency: c.frequency[channel_idx],
            phase_shift: c.phase_shift[channel_idx],
            skew: c.skew[channel_idx],
            smooth_time: c.smooth_time[channel_idx],
        }
    }
}

struct Params {
    shape: LfoShape,
    bipolar: bool,
    steal_phase: bool,
}

impl Params {
    fn from_config(c: &config::LfoConfig) -> Self {
        Self {
            shape: c.shape,
            bipolar: c.bipolar,
            steal_phase: c.steal_phase,
        }
    }
}

struct Voice {
    phase: Phase,
    triggered: bool,
    output: ScalarOutput,
    audio_phase: Phase,
    audio_smoother: Smoother,
    audio_output: Buffer,
}

impl Default for Voice {
    fn default() -> Self {
        Self {
            phase: Phase::ZERO,
            triggered: false,
            output: ScalarOutput::default(),
            audio_phase: Phase::ZERO,
            audio_smoother: Smoother::default(),
            audio_output: zero_buffer(),
        }
    }
}

struct InputBuffers {
    frequency: Buffer,
    phase_shift: Buffer,
    skew: Buffer,
}

type ChannelVoices = [Voice; MAX_VOICES];

pub struct Lfo {
    id: ModuleId,
    params: Params,
    channel_params: [ChannelParams; NUM_CHANNELS],
    inputs: InputBuffers,
    audio_end: AudioEnd,
    ui_end: Option<UiEnd>,
    voices: Box<[ChannelVoices; NUM_CHANNELS]>,
}

impl Lfo {
    pub fn new(id: ModuleId) -> Self {
        Self::from_config(&LfoConfig {
            id,
            ..LfoConfig::default()
        })
    }

    pub fn from_config(config: &config::LfoConfig) -> Self {
        let (audio_end, ui_end) = create_link_pair();

        Self {
            id: config.id,
            params: Params::from_config(config),
            channel_params: array::from_fn(|channel_idx| {
                ChannelParams::from_config(config, channel_idx)
            }),
            inputs: InputBuffers {
                frequency: zero_buffer(),
                phase_shift: zero_buffer(),
                skew: zero_buffer(),
            },
            audio_end,
            ui_end: Some(ui_end),
            voices: new_voices_layout(),
        }
    }

    pub fn get_config(&self) -> LfoConfig {
        LfoConfig {
            id: self.id,
            shape: self.params.shape,
            bipolar: self.params.bipolar,
            steal_phase: self.params.steal_phase,
            frequency: get_stereo_param!(self, frequency),
            phase_shift: get_stereo_param!(self, phase_shift),
            skew: get_stereo_param!(self, skew),
            smooth_time: get_stereo_param!(self, smooth_time),
        }
    }

    set_mono_param!(set_shape, shape, LfoShape);
    set_mono_param!(set_bipolar, bipolar, bool);
    set_mono_param!(set_steal_phase, steal_phase, bool);

    set_stereo_param!(set_frequency, frequency);
    set_stereo_param!(set_phase_shift, phase_shift, phase_shift.clamp(-1.0, 1.0));
    set_stereo_param!(set_skew, skew, skew.clamp(0.0, 1.0));
    set_stereo_param!(set_smooth_time, smooth_time, smooth_time.max(0.0));

    fn triangle(x: Sample) -> Sample {
        2.0 * x.min(1.0 - x)
    }

    fn square(x: Sample) -> Sample {
        Sample::from(x < 0.5)
    }

    fn sine(x: Sample) -> Sample {
        let sine = (f32::consts::PI * x).sin();

        sine * sine
    }

    fn shape_function(shape: LfoShape) -> fn(Sample) -> Sample {
        match shape {
            LfoShape::Triangle => Self::triangle,
            LfoShape::Square => Self::square,
            LfoShape::Sine => Self::sine,
        }
    }

    #[inline]
    fn skew_arg(arg: Sample, skew: Sample) -> Sample {
        let arg_less = arg < skew;

        Sample::from(arg_less) * (0.5 * arg / skew.max(Sample::EPSILON))
            + Sample::from(!arg_less)
                * (0.5 + (arg - skew) * 0.5 / (1.0 - skew).max(Sample::EPSILON))
    }

    #[inline]
    fn apply_bipolar(value: Sample, bipolar: bool) -> Sample {
        Sample::from(bipolar) * value.mul_add(2.0, -1.0) + Sample::from(!bipolar) * value
    }

    fn process_scalar(
        params: &Params,
        channel: &ChannelParams,
        voice: &mut Voice,
        current: bool,
        t_step: Sample,
        router: &mut VoiceRouter<'_, '_>,
    ) {
        let frequency = router.scalar(Input::LowFrequency, channel.frequency, current);

        let phase_shift = router
            .scalar(Input::PhaseShift, channel.phase_shift, current)
            .clamp(-1.0, 1.0);

        let skew = router
            .scalar(Input::Skew, channel.skew, current)
            .clamp(0.0, 1.0);

        let arg = voice.phase.add_normalized(phase_shift).normalized();

        voice.output.advance(Self::apply_bipolar(
            Self::shape_function(params.shape)(Self::skew_arg(arg, skew)),
            params.bipolar,
        ));
        voice.phase.advance_normalized(t_step * frequency);
    }

    fn process_buffer(
        params: &Params,
        channel: &ChannelParams,
        process_params: &ProcessParams,
        inputs: &mut InputBuffers,
        voice: &mut Voice,
        router: &VoiceRouter<'_, '_>,
    ) {
        let frequency_mod = router.buffer(Input::LowFrequency, &mut inputs.frequency);
        let phase_shift_mod = router.buffer(Input::PhaseShift, &mut inputs.phase_shift);
        let skew_mod = router.buffer(Input::Skew, &mut inputs.skew);
        let out = voice.audio_output.iter_mut().take(process_params.samples);

        let shape_func = Self::shape_function(params.shape);
        let freq_phase_mult = Phase::freq_phase_mult(process_params.sample_rate);

        voice
            .audio_smoother
            .update(process_params.sample_rate, channel.smooth_time);

        for (out, frequency_mod, phase_shift_mod, skew_mod) in
            izip!(out, frequency_mod, phase_shift_mod, skew_mod)
        {
            let arg = voice
                .audio_phase
                .add_normalized(channel.phase_shift + phase_shift_mod)
                .normalized();

            let sample = Self::apply_bipolar(
                shape_func(Self::skew_arg(
                    arg,
                    (channel.skew + skew_mod).clamp(0.0, 1.0),
                )),
                params.bipolar,
            );

            *out = voice.audio_smoother.tick(sample);
            voice.audio_phase += (channel.frequency + frequency_mod) * freq_phase_mult;
        }
    }

    fn process_voice(&mut self, router: &mut VoiceRouter<'_, '_>, process_params: &ProcessParams) {
        let channel = &self.channel_params[router.channel_idx()];
        let voice = &mut self.voices[router.channel_idx()][router.voice_idx()];

        if voice.triggered {
            Self::process_scalar(&self.params, channel, voice, false, 0.0, router);
            voice.triggered = false;
        }

        Self::process_scalar(
            &self.params,
            channel,
            voice,
            true,
            process_params.buffer_t_step,
            router,
        );

        Self::process_buffer(
            &self.params,
            channel,
            process_params,
            &mut self.inputs,
            voice,
            router,
        );
    }
}

impl SynthModule for Lfo {
    fn id(&self) -> ModuleId {
        self.id
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::Lfo
    }

    fn inputs(&self) -> &'static [ModInput] {
        static INPUTS: &[ModInput] = &[
            ModInput::scalar(Input::LowFrequency),
            ModInput::scalar(Input::PhaseShift),
            ModInput::scalar(Input::Skew),
        ];

        INPUTS
    }

    fn output(&self) -> DataType {
        DataType::Control
    }

    fn handle_events(&mut self, events: &[VoiceEvent]) {
        for channel in self.voices.iter_mut() {
            for event in events {
                if let VoiceEvent::Trigger {
                    voice_idx,
                    prev_voice_idx,
                    ..
                } = event
                {
                    let (phase, audio_phase) = if let Some(prev_voice_idx) = prev_voice_idx
                        && self.params.steal_phase
                    {
                        let prev_voice = &channel[*prev_voice_idx];
                        (prev_voice.phase, prev_voice.audio_phase)
                    } else {
                        (Phase::ZERO, Phase::ZERO)
                    };

                    let voice = &mut channel[*voice_idx];

                    voice.triggered = true;
                    voice.audio_smoother.reset(0.0);
                    voice.phase = phase;
                    voice.audio_phase = audio_phase;
                }
            }
        }
    }

    fn handle_ui_events(&mut self) {
        while let Some(event) = self.audio_end.pop_event() {
            match event {
                UiEvent::InputParam { input, value } => match input {
                    Input::LowFrequency => self.set_frequency(value),
                    Input::PhaseShift => self.set_phase_shift(value),
                    Input::Skew => self.set_skew(value),
                    _ => (),
                },
                UiEvent::Shape(shape) => self.set_shape(shape),
                UiEvent::Bipolar(value) => self.set_bipolar(value),
                UiEvent::StealPhase(value) => self.set_steal_phase(value),
                UiEvent::SmoothTime(value) => self.set_smooth_time(value),
            }
        }
    }

    fn process(&mut self, process_params: &ProcessParams, router: &mut dyn Router) {
        let mut rf = VoiceRouterFactory::new(self.id, router, process_params);

        for channel_idx in 0..NUM_CHANNELS {
            for (seq_idx, voice_idx) in process_params.active_voices.iter().enumerate() {
                let mut voice_router = rf.for_voice(*voice_idx, channel_idx, seq_idx);

                self.process_voice(&mut voice_router, process_params);
            }
        }
    }

    fn get_buffer_output(&self, voice_idx: usize, channel_idx: usize) -> &Buffer {
        &self.voices[channel_idx][voice_idx].audio_output
    }

    fn get_scalar_output(&self, current: bool, voice_idx: usize, channel_idx: usize) -> Sample {
        self.voices[channel_idx][voice_idx].output.get(current)
    }
}
