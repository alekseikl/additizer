use std::array;

use addi_dsp::fast_exp2_wide;

mod config;
mod link;
pub mod stub;

#[cfg(test)]
mod tests;

pub use config::EnvelopeConfig;
pub use link::{EnvelopeAudioEnd, EnvelopeLinks, EnvelopeUiEnd, UiEvent};

use crate::{
    synth_engine::{
        StereoSample,
        buffer::{VoicesLayout, new_voices_layout},
        routing::{
            ControlRouterType, DataType, Input, InputMeta, InputSlots, MixedSlots, ModuleId,
            ProcessContext, RouterFactory, SamplesOutput, VoiceEvent, VoiceTarget,
        },
        synth_module::SynthModule,
        types::{SIMD_LANES, Smp, WideSmp},
        voices::NUM_CHANNELS,
        voices_handler::DecayingVoice,
    },
    synth_engine::{fill_wide, from_ms},
};

pub const SLOPE_POWER_SCALE: Smp = 20.0;

const MIN_TIME_THRESHOLD: Smp = from_ms(0.5);

struct Params {
    keep_voice_alive: bool,
    steal_level: bool,
    attack_slope: Smp,
    decay_slope: Smp,
    release_slope: Smp,
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
    delay: Smp,
    attack: Smp,
    hold: Smp,
    decay: Smp,
    sustain: Smp,
    release: Smp,
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

/// Envelope stage. Timed variants store normalized progress in `0.0..1.0`.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum Stage {
    Delay(Smp),
    Attack(Smp),
    Hold(Smp),
    Decay(Smp),
    Sustain,
    /// Falling from `level` to 0.
    Release {
        position: Smp,
        level: Smp,
    },
    #[default]
    Done,
}

impl Stage {
    fn position(self) -> Smp {
        match self {
            Stage::Delay(position)
            | Stage::Attack(position)
            | Stage::Hold(position)
            | Stage::Decay(position) => position,
            Stage::Release { position, .. } => position,
            Stage::Sustain | Stage::Done => 0.0,
        }
    }

    fn with_position(self, position: Smp) -> Self {
        match self {
            Stage::Delay(_) => Stage::Delay(position),
            Stage::Attack(_) => Stage::Attack(position),
            Stage::Hold(_) => Stage::Hold(position),
            Stage::Decay(_) => Stage::Decay(position),
            Stage::Release { level, .. } => Stage::Release { position, level },
            Stage::Sustain | Stage::Done => self,
        }
    }

    /// Stage that follows once this one has run to completion.
    fn next(self) -> Self {
        match self {
            Stage::Delay(_) => Stage::Attack(0.0),
            Stage::Attack(_) => Stage::Hold(0.0),
            Stage::Hold(_) => Stage::Decay(0.0),
            Stage::Decay(_) => Stage::Sustain,
            Stage::Release { .. } => Stage::Done,
            Stage::Sustain | Stage::Done => self,
        }
    }

    fn release_started(self) -> bool {
        matches!(self, Stage::Release { .. } | Stage::Done)
    }

    fn is_done(self) -> bool {
        matches!(self, Stage::Done)
    }
}

/// Voice stage published to the UI for painting the phase marker.
#[derive(Clone, Copy, Default)]
pub struct EnvelopePhase {
    pub stage: Stage,
    /// Envelope value at trigger; attack interpolates from this to 1.
    pub start_level: Smp,
}

/// Stage times shorter than [`MIN_TIME_THRESHOLD`] collapse to zero length.
fn stage_time(time: Smp) -> Smp {
    if time < MIN_TIME_THRESHOLD { 0.0 } else { time }
}

fn slope_power(slope: Smp) -> Smp {
    -slope.clamp(-1.0, 1.0) * SLOPE_POWER_SCALE
}

struct FillStage {
    stage: Stage,
    start_level: Smp,
    delay: Smp,
    attack: Smp,
    hold: Smp,
    decay: Smp,
    sustain: Smp,
    release_time: Smp,
    t_step: Smp,
    attack_power: Smp,
    decay_power: Smp,
    release_power: Smp,
}

impl FillStage {
    /// Linear ramp `from + (interval / duration) * t`, one vector of lanes at a time.
    #[cold]
    #[inline(never)]
    fn fill_linear_ramp(out: &mut [Smp], local_t: Smp, t_step: Smp, ramp: Smp, from: Smp) {
        let ramp_x4 = WideSmp::splat(ramp);
        let from_x4 = WideSmp::splat(from);
        let step = WideSmp::splat(t_step * SIMD_LANES as Smp);
        let lanes: [Smp; SIMD_LANES] = array::from_fn(|lane| local_t + lane as Smp * t_step);
        let mut t = WideSmp::new(lanes);

        fill_wide(out, || {
            let y = ramp_x4.mul_add(t, from_x4);
            t += step;
            y
        });
    }

    /// Curved segment, one vector of lanes at a time. Outlined so it stays a leaf.
    #[inline(never)]
    fn fill_curved(
        out: &mut [Smp],
        local_t: Smp,
        t_step: Smp,
        recip: Smp,
        interval: Smp,
        from: Smp,
        power: Smp,
    ) {
        // power_scale(x) = (exp(p * x) - 1) / (exp(p) - 1), so
        // interval * power_scale(x) + from = scale * exp(p * x) + offset.
        let scale = interval / (power.exp() - 1.0);
        let offset = from - scale;
        let dx = t_step * recip;

        const LOG2_E: WideSmp = WideSmp::splat(std::f32::consts::LOG2_E);
        let power_log2e = WideSmp::splat(power) * LOG2_E;
        let step = WideSmp::splat(dx * SIMD_LANES as Smp);
        let scale_x4 = WideSmp::splat(scale);
        let offset_x4 = WideSmp::splat(offset);
        let x0 = local_t * recip;
        let lanes: [Smp; SIMD_LANES] = array::from_fn(|lane| x0 + lane as Smp * dx);
        let mut x = WideSmp::new(lanes);

        fill_wide(out, || {
            let exp = fast_exp2_wide(x * power_log2e);
            let y = scale_x4.mul_add(exp, offset_x4);
            x += step;
            y
        });
    }

    fn duration(&self) -> Smp {
        match self.stage {
            Stage::Delay(_) => self.delay,
            Stage::Attack(_) => self.attack,
            Stage::Hold(_) => self.hold,
            Stage::Decay(_) => self.decay,
            Stage::Release { .. } => self.release_time,
            Stage::Sustain | Stage::Done => 0.0,
        }
    }

    /// Samples still inside the current stage, capped by `max`.
    /// Zero when the stage has no length or less than one sample left.
    fn samples_in_stage(&self, max: usize) -> usize {
        let duration = self.duration();

        if duration <= 0.0 {
            return 0;
        }

        let remaining = (1.0 - self.stage.position()).max(0.0) * duration;
        ((remaining / self.t_step) as usize).min(max)
    }

    fn fill_curve(
        &self,
        out: &mut [Smp],
        local_t: Smp,
        duration: Smp,
        from: Smp,
        to: Smp,
        power: Smp,
    ) {
        let t_step = self.t_step;
        let recip = duration.recip();
        let interval = to - from;

        // Same cutoff as `power_scale`: a flat slope is a linear ramp.
        if power.abs() < 0.005 {
            Self::fill_linear_ramp(out, local_t, t_step, interval * recip, from);
        } else {
            Self::fill_curved(out, local_t, t_step, recip, interval, from, power);
        }
    }

    fn write_stage(&self, out: &mut [Smp]) {
        let position = self.stage.position();

        match self.stage {
            Stage::Delay(_) => out.fill(self.start_level),
            Stage::Attack(_) => self.fill_curve(
                out,
                position * self.attack,
                self.attack,
                self.start_level,
                1.0,
                self.attack_power,
            ),
            Stage::Hold(_) => out.fill(1.0),
            Stage::Decay(_) => self.fill_curve(
                out,
                position * self.decay,
                self.decay,
                1.0,
                self.sustain,
                self.decay_power,
            ),
            Stage::Release { level, .. } => self.fill_curve(
                out,
                position * self.release_time,
                self.release_time,
                level,
                0.0,
                self.release_power,
            ),
            Stage::Sustain | Stage::Done => unreachable!(),
        }
    }

    /// Writes one run of the current stage. A return of 0 means the stage was
    /// advanced and nothing was written (zero length, or a sub-sample remainder).
    fn fill_stage(&mut self, out: &mut [Smp]) -> usize {
        match self.stage {
            Stage::Sustain => {
                out.fill(self.sustain);
                out.len()
            }
            Stage::Done => {
                out.fill(0.0);
                out.len()
            }
            _ => {
                let n = self.samples_in_stage(out.len());

                if n == 0 {
                    self.stage = self.stage.next();
                    return 0;
                }

                self.write_stage(&mut out[..n]);
                let position =
                    (self.stage.position() + n as Smp * self.t_step / self.duration()).min(1.0);
                self.stage = self.stage.with_position(position);
                n
            }
        }
    }

    /// Fills `out`, switching to release at `release_idx` when the note ends.
    /// `level` is the last written sample and the level release starts from.
    fn render(&mut self, out: &mut [Smp], release_idx: Option<usize>, level: &mut Smp) {
        let len = out.len();
        let mut sample_idx = 0;

        while sample_idx < len {
            if release_idx == Some(sample_idx) && !self.stage.release_started() {
                self.stage = Stage::Release {
                    position: 0.0,
                    level: *level,
                };
            }

            let limit = if self.stage.release_started() {
                len
            } else {
                release_idx.unwrap_or(len)
            };
            let span = &mut out[sample_idx..limit];

            if span.is_empty() {
                break;
            }

            let n = self.fill_stage(span);

            if n == 0 {
                continue;
            }

            *level = span[n - 1];
            sample_idx += n;
        }
    }
}

struct Voice {
    stage: Stage,
    /// Pending release event from `process_events`.
    released: Option<usize>,
    /// Last written envelope sample; also the release start level.
    next_frame_value: Smp,
    /// Envelope value at trigger; attack interpolates from this to 1.
    start_level: Smp,
    /// Release has finished. Idle slots start finished so they do not hold a voice.
    done: bool,
}

impl Default for Voice {
    fn default() -> Self {
        Self {
            stage: Stage::Delay(0.0),
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
    set_mono_param!(set_attack_slope, attack_slope, Smp);
    set_mono_param!(set_decay_slope, decay_slope, Smp);
    set_mono_param!(set_release_slope, release_slope, Smp);

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
        let mut stage = voice.stage;
        let mut level = voice.next_frame_value;

        if router.triggered() {
            stage = Stage::Delay(0.0);
            level = start_level;
            voice.done = false;
        }

        let release_idx = voice
            .released
            .take()
            .map(|offset| router.block_to_voice_offset(offset));

        let mut fill = FillStage {
            stage,
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
            attack_power: slope_power(params.attack_slope),
            decay_power: slope_power(params.decay_slope),
            release_power: slope_power(params.release_slope),
        };

        let output = voice_output.output();
        fill.render(output, release_idx, &mut level);

        voice.stage = fill.stage;
        voice.next_frame_value = level;
        if fill.stage.is_done() {
            voice.done = true;
        }

        if router.need_update_ui_mono() {
            self.audio_end.update_phase(EnvelopePhase {
                stage: voice.stage,
                start_level: voice.start_level,
            });
        }
    }
    pub fn process(&mut self, ctx: &mut ProcessContext<L::EngineEnd>) {
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
                        voice.stage = Stage::Delay(0.0);
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
