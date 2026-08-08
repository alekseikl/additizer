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
            ProcessContext, RouterFactory, SamplesOutput, SpectralInputSlot, VoiceEvent,
            VoiceTarget,
        },
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
        }
    }
}

/// Voice clock state published to the UI for painting the phase marker.
#[derive(Clone, Copy)]
pub struct EnvelopePhase {
    /// Seconds since trigger, or since release started once [`Self::released`].
    pub t: Sample,
    pub released: bool,
    pub done: bool,
}

impl Default for EnvelopePhase {
    fn default() -> Self {
        Self {
            t: 0.0,
            released: false,
            done: true,
        }
    }
}

/// Stage times shorter than [`MIN_TIME_THRESHOLD`] collapse to zero length.
fn stage_time(time: Sample) -> Sample {
    if time < MIN_TIME_THRESHOLD { 0.0 } else { time }
}

struct FillStage {
    t: Sample,
    release: Option<Sample>,
    delay: Sample,
    attack: Sample,
    hold: Sample,
    decay: Sample,
    sustain: Sample,
    release_time: Sample,
    t_step: Sample,
    attack_curve: Exponential,
    decay_curve: Exponential,
    release_curve: Exponential,
}

impl FillStage {
    fn samples_until(&self, end: Sample, max: usize) -> usize {
        (((end - self.t).max(0.0) / self.t_step) as usize).min(max)
    }

    fn fill_curve(
        &self,
        out: &mut [Sample],
        mut local_t: Sample,
        duration: Sample,
        from: Sample,
        to: Sample,
        curve: &Exponential,
    ) {
        let recip = duration.recip();
        let interval = to - from;

        for sample in out {
            *sample = interval.mul_add(curve.calc(local_t * recip), from);
            local_t += self.t_step;
        }
    }

    /// Fills the current envelope stage into `out`.
    /// Returns `(samples_written, stage_end)` — `stage_end` is used to snap `t`
    /// forward when a timed stage has a sub-sample remainder (`samples_written == 0`).
    fn fill(&self, out: &mut [Sample]) -> (usize, Option<Sample>) {
        let max = out.len();
        let t = self.t;

        if let Some(from) = self.release {
            return if t < self.release_time {
                let n = self.samples_until(self.release_time, max);
                let out = &mut out[..n];
                self.fill_curve(out, t, self.release_time, from, 0.0, &self.release_curve);
                (n, Some(self.release_time))
            } else {
                out.fill(0.0);
                (max, None)
            };
        }

        let attack_end = self.delay + self.attack;
        let hold_end = attack_end + self.hold;
        let decay_end = hold_end + self.decay;

        if t < self.delay {
            let n = self.samples_until(self.delay, max);
            out[..n].fill(0.0);
            (n, Some(self.delay))
        } else if t < attack_end {
            let n = self.samples_until(attack_end, max);
            let out = &mut out[..n];
            let local_t = t - self.delay;
            self.fill_curve(out, local_t, self.attack, 0.0, 1.0, &self.attack_curve);
            (n, Some(attack_end))
        } else if t < hold_end {
            let n = self.samples_until(hold_end, max);
            out[..n].fill(1.0);
            (n, Some(hold_end))
        } else if t < decay_end {
            let n = self.samples_until(decay_end, max);
            self.fill_curve(
                &mut out[..n],
                t - hold_end,
                self.decay,
                1.0,
                self.sustain,
                &self.decay_curve,
            );
            (n, Some(decay_end))
        } else {
            out.fill(self.sustain);
            (max, None)
        }
    }
}

struct Voice {
    /// Seconds since trigger, or since release started once `release` is set.
    t: Sample,
    /// Envelope value at which the release started.
    release: Option<Sample>,
    /// Pending release event from `process_events`.
    released: Option<usize>,
    /// Last written envelope sample; also the release start level.
    next_frame_value: Sample,
    done: bool,
}

impl Default for Voice {
    fn default() -> Self {
        Self {
            t: 0.0,
            release: None,
            released: None,
            next_frame_value: 0.0,
            done: true,
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
            delay: InputSlots::new(Input::Delay),
            attack: InputSlots::new(Input::Attack),
            hold: InputSlots::new(Input::Hold),
            decay: InputSlots::new(Input::Decay),
            sustain: InputSlots::new(Input::Sustain),
            release: InputSlots::new(Input::Release),
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

pub struct Envelope {
    id: ModuleId,
    params: Params,
    channel_params: [ChannelParams; NUM_CHANNELS],
    audio_end: AudioEnd,
    ui_end: Option<UiEnd>,
    inputs: Inputs,
    output_slot: usize,
    voices: VoicesLayout<Voice>,
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

    fn process_voice(
        &mut self,
        target: &VoiceTarget,
        outputs: &mut VoicesLayout<SamplesOutput>,
        rf: &mut RouterFactory<ControlRouterType>,
    ) {
        let channel_idx = target.channel_idx;
        let voice_idx = target.voice_idx;
        let (mut router, mut voice_output) = rf.for_voice(target, outputs);
        let inputs = &self.inputs;
        let params = &self.params;
        let channel = &self.channel_params[channel_idx];
        let voice = &mut self.voices[channel_idx][voice_idx];
        let t_step = router.sample_rate().recip();

        if router.triggered() {
            voice.t = 0.0;
            voice.release = None;
            voice.next_frame_value = 0.0;
            voice.done = false;
        }

        let release_idx = voice
            .released
            .take()
            .map(|offset| router.block_to_voice_offset(offset));

        let mut fill = FillStage {
            t: voice.t,
            release: voice.release,
            delay: stage_time(router.scalar(&inputs.delay, channel.delay)),
            attack: stage_time(router.scalar(&inputs.attack, channel.attack)),
            hold: stage_time(router.scalar(&inputs.hold, channel.hold)),
            decay: stage_time(router.scalar(&inputs.decay, channel.decay)),
            sustain: router
                .scalar(&inputs.sustain, channel.sustain)
                .clamp(0.0, 1.0),
            release_time: stage_time(router.scalar(&inputs.release, channel.release)),
            t_step,
            attack_curve: Exponential::new(params.attack_curvature),
            decay_curve: Exponential::new(params.decay_curvature),
            release_curve: Exponential::new(params.release_curvature),
        };

        let mut sample_idx = 0;
        let output = voice_output.output();
        let len = output.len();

        while sample_idx < len {
            if release_idx == Some(sample_idx) && fill.release.is_none() {
                fill.release = Some(voice.next_frame_value);
                fill.t = 0.0;
            }

            let limit = if fill.release.is_none() {
                release_idx.unwrap_or(len)
            } else {
                len
            };

            let (n, stage_end) = fill.fill(&mut output[sample_idx..limit]);

            if n == 0 {
                // Sub-sample remainder of a stage — snap to its boundary.
                if let Some(end) = stage_end {
                    fill.t = end;
                }
                continue;
            }

            voice.next_frame_value = output[sample_idx + n - 1];
            fill.t += n as Sample * fill.t_step;
            sample_idx += n;
        }

        voice.t = fill.t;
        voice.release = fill.release;
        voice.done = fill.release.is_some_and(|_| fill.t >= fill.release_time);

        if router.need_update_ui_mono() {
            self.audio_end.update_phase(EnvelopePhase {
                t: voice.t,
                released: voice.release.is_some(),
                done: voice.done,
            });
        }
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
                        channel[*voice_idx].released = None;
                        channel[*voice_idx].done = false;
                    }
                    VoiceEvent::Release {
                        voice_idx, offset, ..
                    } => {
                        channel[*voice_idx].released = Some(*offset);
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

                    if !voice.done {
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
                UiEvent::AttackCurvature(value) => self.set_attack_curvature(value),
                UiEvent::DecayCurvature(value) => self.set_decay_curvature(value),
                UiEvent::ReleaseCurvature(value) => self.set_release_curvature(value),
                UiEvent::KeepVoiceAlive(value) => self.set_keep_voice_alive(value),
            }
        }
    }

    fn process(&mut self, ctx: &mut ProcessContext) {
        ctx.for_control(self.id, self.output_slot, |rf, target, outputs| {
            self.process_voice(target, outputs, rf);
        });
    }
}
