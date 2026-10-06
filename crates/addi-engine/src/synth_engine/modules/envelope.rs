use std::array;

mod config;
mod link;
pub mod stub;

#[cfg(test)]
mod tests;

pub use config::EnvelopeConfig;
pub use link::{EnvelopeAudioEnd, EnvelopeLinks, EnvelopeUiEnd, UiEvent};

use crate::{
    synth_engine::from_ms,
    synth_engine::{
        StereoSample,
        buffer::{VoicesLayout, new_voices_layout},
        curves::{CurveFunction, Exponential},
        routing::{
            ControlRouterType, DataType, Input, InputMeta, InputSlots, MixedSlots, ModuleId,
            NUM_CHANNELS, ProcessContext, RouterFactory, SamplesOutput, VoiceEvent, VoiceTarget,
        },
        synth_module::SynthModule,
        types::Sample,
        voices_handler::DecayingVoice,
    },
};

pub const SLOPE_POWER_SCALE: Sample = 20.0;

const MIN_TIME_THRESHOLD: Sample = from_ms(0.5);

struct Params {
    keep_voice_alive: bool,
    steal_level: bool,
    attack_slope: Sample,
    decay_slope: Sample,
    release_slope: Sample,
}

impl Params {
    fn from_config(c: &config::EnvelopeConfig) -> Self {
        Self {
            keep_voice_alive: c.keep_voice_alive,
            steal_level: c.steal_level,
            attack_slope: c.attack_slope,
            decay_slope: c.decay_slope,
            release_slope: c.release_slope,
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
    /// Envelope value at trigger; attack interpolates from this to 1.
    pub start_level: Sample,
    pub released: bool,
    pub done: bool,
}

impl Default for EnvelopePhase {
    fn default() -> Self {
        Self {
            t: 0.0,
            start_level: 0.0,
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
    start_level: Sample,
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
            out[..n].fill(self.start_level);
            (n, Some(self.delay))
        } else if t < attack_end {
            let n = self.samples_until(attack_end, max);
            let out = &mut out[..n];
            let local_t = t - self.delay;
            self.fill_curve(
                out,
                local_t,
                self.attack,
                self.start_level,
                1.0,
                &self.attack_curve,
            );
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
    /// Envelope value at trigger; attack interpolates from this to 1.
    start_level: Sample,
    done: bool,
}

impl Default for Voice {
    fn default() -> Self {
        Self {
            t: 0.0,
            release: None,
            released: None,
            next_frame_value: 0.0,
            start_level: 0.0,
            done: true,
        }
    }
}

pub struct Inputs {
    delay: MixedSlots,
    attack: MixedSlots,
    hold: MixedSlots,
    decay: MixedSlots,
    sustain: MixedSlots,
    release: MixedSlots,
}

impl Default for Inputs {
    fn default() -> Self {
        Self {
            delay: MixedSlots::new(Input::Delay),
            attack: MixedSlots::new(Input::Attack),
            hold: MixedSlots::new(Input::Hold),
            decay: MixedSlots::new(Input::Decay),
            sustain: MixedSlots::new(Input::Sustain),
            release: MixedSlots::new(Input::Release),
        }
    }
}

impl Inputs {
    fn from_slots(inputs: &[InputSlots]) -> Self {
        let mut result = Self::default();

        for input in inputs {
            let InputSlots::Mixed(input) = input else {
                continue;
            };

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

pub struct Envelope<L: EnvelopeLinks = stub::Links> {
    id: ModuleId,
    params: Params,
    channel_params: [ChannelParams; NUM_CHANNELS],
    audio_end: L::AudioEnd,
    ui_end: Option<L::UiEnd>,
    inputs: Inputs,
    output_slot: usize,
    voices: VoicesLayout<Voice>,
}

impl<L: EnvelopeLinks> Envelope<L> {
    pub fn take_ui_end(&mut self) -> Option<L::UiEnd> {
        self.ui_end.take()
    }

    pub fn new(id: ModuleId) -> Self {
        Self::from_config(&EnvelopeConfig {
            id,
            ..EnvelopeConfig::default()
        })
    }

    pub fn from_config(config: &config::EnvelopeConfig) -> Self {
        let (audio_end, ui_end) = L::create_link_pair();

        Self {
            id: config.id,
            params: Params::from_config(config),
            channel_params: array::from_fn(|channel_idx| {
                ChannelParams::from_config(config, channel_idx)
            }),
            audio_end,
            ui_end,
            inputs: Inputs::default(),
            output_slot: usize::MAX,
            voices: new_voices_layout(),
        }
    }

    pub fn get_config(&self) -> EnvelopeConfig {
        EnvelopeConfig {
            id: self.id,
            keep_voice_alive: self.params.keep_voice_alive,
            steal_level: self.params.steal_level,
            delay: get_stereo_param!(self, delay),
            attack: get_stereo_param!(self, attack),
            attack_slope: self.params.attack_slope,
            hold: get_stereo_param!(self, hold),
            decay: get_stereo_param!(self, decay),
            decay_slope: self.params.decay_slope,
            sustain: get_stereo_param!(self, sustain),
            release: get_stereo_param!(self, release),
            release_slope: self.params.release_slope,
        }
    }

    set_mono_param!(set_keep_voice_alive, keep_voice_alive, bool);
    set_mono_param!(set_steal_level, steal_level, bool);
    set_mono_param!(set_attack_slope, attack_slope, Sample);
    set_mono_param!(set_decay_slope, decay_slope, Sample);
    set_mono_param!(set_release_slope, release_slope, Sample);

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
        rf: &mut RouterFactory<ControlRouterType, L::EngineEnd>,
    ) {
        let channel_idx = target.channel_idx;
        let voice_idx = target.voice_idx;
        let (mut router, mut voice_output) = rf.for_voice(target, outputs);
        let inputs = &self.inputs;
        let params = &self.params;
        let channel = &self.channel_params[channel_idx];
        let voice = self.voices.at_mut(channel_idx, voice_idx);
        let t_step = router.sample_rate().recip();
        let start_level = voice.start_level;

        if router.triggered() {
            voice.t = 0.0;
            voice.release = None;
            voice.next_frame_value = start_level;
            voice.done = false;
        }

        let release_idx = voice
            .released
            .take()
            .map(|offset| router.block_to_voice_offset(offset));

        let mut fill = FillStage {
            t: voice.t,
            release: voice.release,
            start_level,
            delay: stage_time(router.scalar(&inputs.delay, channel.delay)),
            attack: stage_time(router.scalar(&inputs.attack, channel.attack)),
            hold: stage_time(router.scalar(&inputs.hold, channel.hold)),
            decay: stage_time(router.scalar(&inputs.decay, channel.decay)),
            sustain: router
                .scalar(&inputs.sustain, channel.sustain)
                .clamp(0.0, 1.0),
            release_time: stage_time(router.scalar(&inputs.release, channel.release)),
            t_step,
            attack_curve: Exponential::new(params.attack_slope),
            decay_curve: Exponential::new(params.decay_slope),
            release_curve: Exponential::new(params.release_slope),
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
                start_level: voice.start_level,
                released: voice.release.is_some(),
                done: voice.done,
            });
        }
    }
    pub(crate) fn process(&mut self, ctx: &mut ProcessContext<L::EngineEnd>) {
        ctx.control(self.id, self.output_slot)
            .for_voices(|rf, target, outputs| {
                self.process_voice(target, outputs, rf);
            });
    }
}

impl<L: EnvelopeLinks> SynthModule for Envelope<L> {
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

    fn set_input_slots(&mut self, inputs: &[InputSlots]) {
        self.inputs = Inputs::from_slots(inputs);
    }

    fn update_input_amount(&mut self, input_type: Input, src_slot: usize, amount: StereoSample) {
        self.inputs.update_amount(input_type, src_slot, amount);
    }

    fn process_events(&mut self, events: &[VoiceEvent]) {
        for event in events {
            match event {
                VoiceEvent::Reset {
                    voice_idx,
                    replaced_voice_idx,
                    ..
                } => {
                    for channel_idx in 0..NUM_CHANNELS {
                        let start_level = if let Some(replaced_voice_idx) = replaced_voice_idx
                            && self.params.steal_level
                        {
                            self.voices
                                .at(channel_idx, *replaced_voice_idx)
                                .next_frame_value
                        } else {
                            0.0
                        };

                        let voice = self.voices.at_mut(channel_idx, *voice_idx);
                        voice.released = None;
                        voice.done = false;
                        voice.start_level = start_level;
                    }
                }
                VoiceEvent::Release {
                    voice_idx, offset, ..
                } => {
                    for voice in self.voices.channels_at_mut(*voice_idx) {
                        voice.released = Some(*offset);
                    }
                }
                _ => (),
            }
        }
    }

    fn poll_decaying_voices(&self, decaying_voices: &mut [DecayingVoice]) {
        if self.params.keep_voice_alive {
            for decaying in decaying_voices.iter_mut().filter(|d| d.is_done()) {
                for voice in self.voices.channels_at(decaying.index()) {
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
                UiEvent::AttackSlope(value) => self.set_attack_slope(value),
                UiEvent::DecaySlope(value) => self.set_decay_slope(value),
                UiEvent::ReleaseSlope(value) => self.set_release_slope(value),
                UiEvent::KeepVoiceAlive(value) => self.set_keep_voice_alive(value),
                UiEvent::StealLevel(value) => self.set_steal_level(value),
            }
        }
    }
}
