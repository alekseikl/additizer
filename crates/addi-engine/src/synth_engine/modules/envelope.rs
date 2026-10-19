use std::array;

mod config;
mod link;
mod stage_render;
pub mod stub;

#[cfg(test)]
mod tests;

pub use config::EnvelopeConfig;
pub use link::{EnvelopeAudioEnd, EnvelopeLinks, EnvelopeUiEnd, UiEvent};
use stage_render::{render_curve, render_level};

use crate::{
    synth_engine::from_ms,
    synth_engine::{
        StereoSample,
        buffer::{VoicesLayout, new_voices_layout},
        routing::{
            ControlRouterType, DataType, Input, InputMeta, InputSlots, MixedSlots, ModuleId,
            ProcessContext, RouterFactory, SamplesOutput, VoiceEvent, VoiceRouter, VoiceTarget,
        },
        synth_module::SynthModule,
        types::Smp,
        voices::NUM_CHANNELS,
        voices_handler::DecayingVoice,
    },
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

struct StageCtx<'a> {
    t_step: Smp,
    start_level: Smp,
    params: &'a Params,
    inputs: &'a Inputs,
    channel: &'a ChannelParams,
}

#[derive(Default)]
struct Voice {
    /// Idle slots start at [`Stage::Done`] so they do not hold a voice.
    stage: Stage,
    /// Pending release event from `process_events`.
    released: Option<usize>,
    /// Last written envelope sample; also the release start level.
    next_frame_value: Smp,
    /// Envelope value at trigger; attack interpolates from this to 1.
    start_level: Smp,
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

    /// Renders `stage` into `out`.
    ///
    /// `Ok(stage)` — `out` is filled and this stage is still running.
    /// `Err((next, written))` — this stage ended after `written` samples
    /// (`0` when it had no time left).
    fn render<'v, 'f, 'c>(
        stage: Stage,
        out: &mut [Smp],
        ctx: &StageCtx<'_>,
        router: &mut VoiceRouter<'v, 'f, 'c, ControlRouterType, L::EngineEnd>,
    ) -> Result<Stage, (Stage, usize)> {
        let t_step = ctx.t_step;
        let inputs = ctx.inputs;
        let channel = ctx.channel;

        match stage {
            Stage::Sustain => {
                let sustain = router
                    .scalar(&inputs.sustain, channel.sustain)
                    .clamp(0.0, 1.0);
                out.fill(sustain);
                Ok(Stage::Sustain)
            }
            Stage::Done => {
                out.fill(0.0);
                Ok(Stage::Done)
            }
            Stage::Delay(position) => {
                let delay = stage_time(router.scalar(&inputs.delay, channel.delay));
                render_level(out, position, delay, t_step, ctx.start_level)
                    .map(Stage::Delay)
                    .map_err(|n| (Stage::Attack(0.0), n))
            }
            Stage::Attack(position) => {
                let attack = stage_time(router.scalar(&inputs.attack, channel.attack));
                render_curve(
                    out,
                    position,
                    attack,
                    t_step,
                    ctx.start_level,
                    1.0,
                    slope_power(ctx.params.attack_slope),
                )
                .map(Stage::Attack)
                .map_err(|n| (Stage::Hold(0.0), n))
            }
            Stage::Hold(position) => {
                let hold = stage_time(router.scalar(&inputs.hold, channel.hold));
                render_level(out, position, hold, t_step, 1.0)
                    .map(Stage::Hold)
                    .map_err(|n| (Stage::Decay(0.0), n))
            }
            Stage::Decay(position) => {
                let decay = stage_time(router.scalar(&inputs.decay, channel.decay));
                let sustain = router
                    .scalar(&inputs.sustain, channel.sustain)
                    .clamp(0.0, 1.0);
                render_curve(
                    out,
                    position,
                    decay,
                    t_step,
                    1.0,
                    sustain,
                    slope_power(ctx.params.decay_slope),
                )
                .map(Stage::Decay)
                .map_err(|n| (Stage::Sustain, n))
            }
            Stage::Release { position, level } => {
                let release = stage_time(router.scalar(&inputs.release, channel.release));
                render_curve(
                    out,
                    position,
                    release,
                    t_step,
                    level,
                    0.0,
                    slope_power(ctx.params.release_slope),
                )
                .map(|position| Stage::Release { position, level })
                .map_err(|n| (Stage::Done, n))
            }
        }
    }

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
        let start_level = voice.start_level;
        let triggered = router.triggered();
        let mut stage = voice.stage;

        if triggered {
            stage = Stage::Delay(0.0);
        }

        let release_idx = voice
            .released
            .take()
            .map(|offset| router.block_to_voice_offset(offset));
        let ctx = StageCtx {
            t_step: router.sample_rate().recip(),
            start_level,
            params,
            inputs,
            channel,
        };
        let output = voice_output.output();
        let len = output.len();
        let mut pos = 0;
        let mut limit = release_idx.unwrap_or(len);

        loop {
            while pos < limit {
                stage = match Self::render(stage, &mut output[pos..limit], &ctx, &mut router) {
                    Ok(next) => {
                        pos = limit;
                        next
                    }
                    Err((next, written)) => {
                        pos += written;
                        next
                    }
                };
            }

            if limit == len {
                break;
            }

            stage = Stage::Release {
                position: 0.0,
                level: pos
                    .checked_sub(1)
                    .map(|i| output[i])
                    .unwrap_or(if triggered {
                        start_level
                    } else {
                        voice.next_frame_value
                    }),
            };
            limit = len;
        }

        voice.stage = stage;
        voice.next_frame_value = output.last().copied().unwrap_or_default();

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
                    if voice.stage != Stage::Done {
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
