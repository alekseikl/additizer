use std::array;

mod config;
mod link;
mod ui_bridge;

pub use config::EnvelopeConfig;
use link::{AudioEnd, UiEnd, UiEvent, create_link_pair};
pub use ui_bridge::EnvelopeUiBridge;

use crate::{
    synth_engine::{
        StereoSample,
        buffer::{VoicesLayout, new_voices_layout},
        curves::{CurveFunction, Exponential},
        routing::{
            ControlRouterType, DataType, Input, InputMeta, InputSlots, ModuleId, NUM_CHANNELS,
            ProcessContext, SamplesOutput, SpectralInputSlot, VoiceEvent, VoiceRouter,
        },
        smooth::Smoother,
        synth_module::SynthModule,
        types::Sample,
        voices_handler::DecayingVoice,
    },
    utils::from_ms,
};

const MIN_TIME_THRESHOLD: Sample = from_ms(0.5);

struct Params {
    keep_voice_alive: bool,
    attack_curvature: Sample,
    decay_curvature: Sample,
    release_curvature: Sample,
}

impl Params {
    fn from_config(c: &config::EnvelopeConfig) -> Self {
        Self {
            keep_voice_alive: c.keep_voice_alive,
            attack_curvature: c.attack_curvature,
            decay_curvature: c.decay_curvature,
            release_curvature: c.release_curvature,
        }
    }
}

struct ChannelParams {
    delay: Sample,
    attack: Sample,
    hold: Sample,
    decay: Sample,
    sustain: Sample,
    release: Sample,
    smooth: Sample,
}

impl ChannelParams {
    fn from_config(c: &EnvelopeConfig, channel_idx: usize) -> Self {
        Self {
            delay: c.delay[channel_idx],
            attack: c.attack[channel_idx],
            hold: c.hold[channel_idx],
            decay: c.decay[channel_idx],
            sustain: c.sustain[channel_idx],
            release: c.release[channel_idx],
            smooth: c.smooth[channel_idx],
        }
    }
}

enum CurveBlockResult {
    HasMore,
    Done,
}

struct CurveIter {
    curve_fn: Exponential,
    t: Sample,
    value_from: Sample,
    interval: Sample,
}

impl CurveIter {
    fn new(curvature: Sample, from: Sample, to: Sample) -> Self {
        let from = from.clamp(0.0, 1.0);
        let to = to.clamp(0.0, 1.0);
        Self {
            curve_fn: Exponential::new(curvature),
            t: 0.0,
            value_from: from,
            interval: to - from,
        }
    }

    fn flat(level: Sample) -> Self {
        Self::new(0.0, level, level)
    }

    fn next_block(
        &mut self,
        t_step: Sample,
        time: Sample,
        sample_from: &mut usize,
        output: &mut [Sample],
    ) -> CurveBlockResult {
        if time < MIN_TIME_THRESHOLD {
            return CurveBlockResult::Done;
        }

        let output = &mut output[*sample_from..];

        let samples = output
            .len()
            .min(((time - self.t).max(0.0) / t_step) as usize);
        let time_recip = time.recip();

        for out in output.iter_mut().take(samples) {
            *out = self
                .interval
                .mul_add(self.curve_fn.calc(self.t * time_recip), self.value_from);
            self.t += t_step;
        }

        *sample_from += samples;

        if samples < output.len() {
            CurveBlockResult::Done
        } else {
            CurveBlockResult::HasMore
        }
    }
}

enum Stage {
    Delay(CurveIter),
    Attack(CurveIter),
    Hold(CurveIter),
    Decay(CurveIter),
    Sustain,
    Release(CurveIter),
    Flush(CurveIter),
    Done,
}

struct VoiceState {
    stage: Stage,
    triggered: bool,
    released: bool,
    next_frame_value: Sample,
    smoother: Smoother,
}

impl Default for VoiceState {
    fn default() -> Self {
        Self {
            stage: Stage::Done,
            triggered: false,
            released: false,
            next_frame_value: 0.0,
            smoother: Smoother::default(),
        }
    }
}

pub struct Inputs {
    delay: InputSlots,
    attack: InputSlots,
    hold: InputSlots,
    decay: InputSlots,
    sustain: InputSlots,
    release: InputSlots,
}

impl Default for Inputs {
    fn default() -> Self {
        Self {
            delay: InputSlots::empty(Input::Delay),
            attack: InputSlots::empty(Input::Attack),
            hold: InputSlots::empty(Input::Hold),
            decay: InputSlots::empty(Input::Decay),
            sustain: InputSlots::empty(Input::Sustain),
            release: InputSlots::empty(Input::Release),
        }
    }
}

impl Inputs {
    fn from_slots(inputs: &[InputSlots], _spectral_inputs: &[SpectralInputSlot]) -> Self {
        let mut result = Self::default();

        for input in inputs {
            match input.input_type {
                Input::Delay => result.delay = input.clone(),
                Input::Attack => result.attack = input.clone(),
                Input::Hold => result.hold = input.clone(),
                Input::Decay => result.decay = input.clone(),
                Input::Sustain => result.sustain = input.clone(),
                Input::Release => result.release = input.clone(),
                _ => (),
            }
        }

        result
    }

    fn update_amount(&mut self, input_type: Input, src_slot: usize, amount: StereoSample) {
        match input_type {
            Input::Delay => self.delay.update_amount(src_slot, amount),
            Input::Attack => self.attack.update_amount(src_slot, amount),
            Input::Hold => self.hold.update_amount(src_slot, amount),
            Input::Decay => self.decay.update_amount(src_slot, amount),
            Input::Sustain => self.sustain.update_amount(src_slot, amount),
            Input::Release => self.release.update_amount(src_slot, amount),
            _ => (),
        }
    }
}

type Router<'v, 'f, 'c> = VoiceRouter<'v, 'f, 'c, ControlRouterType>;

pub struct Envelope {
    id: ModuleId,
    params: Params,
    channel_params: [ChannelParams; NUM_CHANNELS],
    audio_end: AudioEnd,
    ui_end: Option<UiEnd>,
    inputs: Inputs,
    output_slot: usize,
    voices: VoicesLayout<VoiceState>,
}

impl Envelope {
    pub fn new(id: ModuleId) -> Self {
        Self::from_config(&EnvelopeConfig {
            id,
            ..EnvelopeConfig::default()
        })
    }

    pub fn from_config(config: &config::EnvelopeConfig) -> Self {
        let (audio_end, ui_end) = create_link_pair();

        Self {
            id: config.id,
            params: Params::from_config(config),
            channel_params: array::from_fn(|channel_idx| {
                ChannelParams::from_config(config, channel_idx)
            }),
            audio_end,
            ui_end: Some(ui_end),
            inputs: Inputs::default(),
            output_slot: usize::MAX,
            voices: new_voices_layout(),
        }
    }

    pub fn get_config(&self) -> EnvelopeConfig {
        EnvelopeConfig {
            id: self.id,
            keep_voice_alive: self.params.keep_voice_alive,
            delay: get_stereo_param!(self, delay),
            attack: get_stereo_param!(self, attack),
            attack_curvature: self.params.attack_curvature,
            hold: get_stereo_param!(self, hold),
            decay: get_stereo_param!(self, decay),
            decay_curvature: self.params.decay_curvature,
            sustain: get_stereo_param!(self, sustain),
            release: get_stereo_param!(self, release),
            release_curvature: self.params.release_curvature,
            smooth: get_stereo_param!(self, smooth),
        }
    }

    set_mono_param!(set_keep_voice_alive, keep_voice_alive, bool);
    set_mono_param!(set_attack_curvature, attack_curvature, Sample);
    set_mono_param!(set_decay_curvature, decay_curvature, Sample);
    set_mono_param!(set_release_curvature, release_curvature, Sample);

    set_stereo_param!(set_delay, delay);
    set_stereo_param!(set_attack, attack);
    set_stereo_param!(set_hold, hold);
    set_stereo_param!(set_decay, decay);
    set_stereo_param!(set_sustain, sustain);
    set_stereo_param!(set_release, release);
    set_stereo_param!(set_smooth, smooth);

    fn process_voice(
        &mut self,
        output_slot: &mut VoicesLayout<SamplesOutput>,
        mut router: Router<'_, '_, '_>,
    ) {
        let channel_idx = router.channel_idx();
        let voice_idx = router.voice_idx();
        let inputs = &self.inputs;
        let params = &self.params;
        let channel = &mut self.channel_params[channel_idx];
        let voice = &mut self.voices[channel_idx][voice_idx];
        let samples = router.samples();
        let t_step = router.sample_rate().recip();
        let voice_output = &mut output_slot[channel_idx][voice_idx];

        if voice.triggered {
            voice.next_frame_value = 0.0;
            voice.smoother.reset(0.0);
            voice.stage = Stage::Delay(CurveIter::flat(0.0));
        }

        if voice.released {
            voice.stage = Stage::Release(CurveIter::new(
                params.release_curvature,
                voice.next_frame_value,
                0.0,
            ));
            voice.released = false;
        }

        let mut control_output = voice_output.control_output(samples, voice.triggered);
        let output = control_output.output();
        let mut sample_from = 0;

        loop {
            voice.stage = match &mut voice.stage {
                Stage::Delay(curve) => {
                    match curve.next_block(
                        t_step,
                        router.scalar_param(&inputs.delay, channel.delay, false),
                        &mut sample_from,
                        output,
                    ) {
                        CurveBlockResult::Done => {
                            Stage::Attack(CurveIter::new(params.attack_curvature, 0.0, 1.0))
                        }
                        CurveBlockResult::HasMore => break,
                    }
                }
                Stage::Attack(curve) => {
                    match curve.next_block(
                        t_step,
                        router.scalar_param(&inputs.attack, channel.attack, false),
                        &mut sample_from,
                        output,
                    ) {
                        CurveBlockResult::Done => Stage::Hold(CurveIter::flat(1.0)),
                        CurveBlockResult::HasMore => break,
                    }
                }
                Stage::Hold(curve) => {
                    match curve.next_block(
                        t_step,
                        router.scalar_param(&inputs.hold, channel.hold, false),
                        &mut sample_from,
                        output,
                    ) {
                        CurveBlockResult::Done => Stage::Decay(CurveIter::new(
                            params.decay_curvature,
                            1.0,
                            channel.sustain,
                        )),
                        CurveBlockResult::HasMore => break,
                    }
                }
                Stage::Decay(curve) => {
                    match curve.next_block(
                        t_step,
                        router.scalar_param(&inputs.decay, channel.decay, false),
                        &mut sample_from,
                        output,
                    ) {
                        CurveBlockResult::Done => Stage::Sustain,
                        CurveBlockResult::HasMore => break,
                    }
                }
                Stage::Sustain => {
                    output[sample_from..].fill(
                        router
                            .scalar_param(&inputs.sustain, channel.sustain, false)
                            .clamp(0.0, 1.0),
                    );
                    break;
                }
                Stage::Release(curve) => {
                    match curve.next_block(
                        t_step,
                        router.scalar_param(&inputs.release, channel.release, false),
                        &mut sample_from,
                        output,
                    ) {
                        CurveBlockResult::Done => Stage::Flush(CurveIter::flat(0.0)),
                        CurveBlockResult::HasMore => break,
                    }
                }
                Stage::Flush(curve) => {
                    match curve.next_block(t_step, channel.smooth, &mut sample_from, output) {
                        CurveBlockResult::Done => Stage::Done,
                        CurveBlockResult::HasMore => break,
                    }
                }
                Stage::Done => {
                    output[sample_from..].fill(0.0);
                    break;
                }
            };
        }

        voice.next_frame_value = control_output.next_frame_value();
        voice.triggered = false;

        drop(control_output);

        voice.smoother.apply_if_needed(
            samples,
            router.sample_rate(),
            channel.smooth,
            voice_output.output(samples),
        );
    }
}

impl SynthModule for Envelope {
    fn id(&self) -> ModuleId {
        self.id
    }

    fn inputs(&self) -> &'static [InputMeta] {
        static INPUTS: &[InputMeta] = &[
            InputMeta::control(Input::Delay),
            InputMeta::control(Input::Attack),
            InputMeta::control(Input::Hold),
            InputMeta::control(Input::Decay),
            InputMeta::control(Input::Sustain),
            InputMeta::control(Input::Release),
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
        for channel in self.voices.iter_mut() {
            for event in events {
                match event {
                    VoiceEvent::Trigger { voice_idx, .. } => {
                        channel[*voice_idx].triggered = true;
                        channel[*voice_idx].released = false;
                    }
                    VoiceEvent::Release { voice_idx, .. } => {
                        channel[*voice_idx].released = true;
                    }
                    _ => (),
                }
            }
        }
    }

    fn poll_decaying_voices(&self, decaying_voices: &mut [DecayingVoice]) {
        if self.params.keep_voice_alive {
            for decaying in decaying_voices.iter_mut().filter(|d| d.is_done()) {
                for channel in self.voices.iter() {
                    let voice = &channel[decaying.index()];

                    if !matches!(voice.stage, Stage::Done) || voice.triggered {
                        decaying.mark_active();
                    }
                }
            }
        }
    }

    fn process_ui_events(&mut self) {
        while let Some(event) = self.audio_end.pop_event() {
            match event {
                UiEvent::InputParam { input, value } => match input {
                    Input::Delay => self.set_delay(value),
                    Input::Attack => self.set_attack(value),
                    Input::Hold => self.set_hold(value),
                    Input::Decay => self.set_decay(value),
                    Input::Sustain => self.set_sustain(value),
                    Input::Release => self.set_release(value),
                    _ => (),
                },
                UiEvent::Smooth(value) => self.set_smooth(value),
                UiEvent::AttackCurvature(value) => self.set_attack_curvature(value),
                UiEvent::DecayCurvature(value) => self.set_decay_curvature(value),
                UiEvent::ReleaseCurvature(value) => self.set_release_curvature(value),
                UiEvent::KeepVoiceAlive(value) => self.set_keep_voice_alive(value),
            }
        }
    }

    fn process(&mut self, ctx: &mut ProcessContext) {
        ctx.for_control(self.id, self.output_slot, |router, output| {
            let num_active_voices = router.params().active_voices.len();

            for channel_idx in 0..NUM_CHANNELS {
                for seq_idx in 0..num_active_voices {
                    let voice_idx = router.params().active_voices[seq_idx];

                    self.process_voice(output, router.for_voice(channel_idx, voice_idx, seq_idx));
                }
            }
        });
    }
}
