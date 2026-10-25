use std::array;

mod config;
mod link;
pub mod stub;
mod timebase;

#[cfg(test)]
mod tests;

pub use config::SegmentsEnvelopeConfig;
use config::{MAX_RATE, MIN_RATE};
pub use link::{SegmentsEnvelopeAudioEnd, SegmentsEnvelopeLinks, SegmentsEnvelopeUiEnd, UiEvent};
pub use timebase::Timebase;

use addi_dsp::{segments::Segments, utils::sanitize};

use crate::synth_engine::{
    Input, ModuleId, SmoothedSampleParams, Smp, StereoSample,
    buffer::{Buffer, VoicesLayout, new_voices_layout, zero_buffer},
    routing::{
        ControlRouterType, DataType, InputMeta, InputSlots, MixedSlots, ProcessContext,
        RouterFactory, SamplesOutput, VoiceEvent, VoiceTarget,
    },
    smooth::SmoothedSample,
    synth_module::SynthModule,
    voices::NUM_CHANNELS,
    voices_handler::DecayingVoice,
};

struct ChannelParams {
    rate: SmoothedSample,
}

impl ChannelParams {
    fn from_config(c: &config::SegmentsEnvelopeConfig, channel_idx: usize) -> Self {
        Self {
            rate: c.rate[channel_idx].into(),
        }
    }

    pub fn advance_smoothers(&mut self, smooth_params: &SmoothedSampleParams, samples: usize) {
        self.rate.advance(smooth_params, samples);
    }
}

struct Params {
    segments: Segments,
    steal_level: bool,
    keep_voice_alive: bool,
    timebase: Timebase,
}

impl Params {
    fn from_config(c: &config::SegmentsEnvelopeConfig) -> Self {
        Self {
            segments: c.segments.clone(),
            steal_level: c.steal_level,
            keep_voice_alive: c.keep_voice_alive,
            timebase: c.timebase,
        }
    }
}

/// Playhead published for the editor. `start_level` is the level the first
/// segment leaves; it differs from the drawn start only after a stolen note.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SegmentsEnvelopePhase {
    pub phase: Smp,
    pub start_level: Smp,
}

/// One pass of the curve. A voice starts done and returns there when the pass finishes.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
enum Shot {
    Play(Smp),
    /// The pass finished and the last level is held.
    #[default]
    Done,
}

impl Shot {
    fn phase(self) -> Smp {
        match self {
            Self::Play(phase) => phase,
            Self::Done => 1.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Voice {
    shot: Shot,
    /// Level the first segment leaves. The drawn start, unless this note stole one.
    start_level: Smp,
    /// Last sample this voice wrote. A stolen note copies it.
    level: Smp,
}

pub struct Inputs {
    rate: MixedSlots,
}

impl Default for Inputs {
    fn default() -> Self {
        Self {
            rate: MixedSlots::new(Input::Rate),
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

            if input.input_type == Input::Rate {
                result.rate = input.clone();
            }
        }

        result
    }

    fn update_amount(&mut self, input_type: Input, src_slot: usize, amount: StereoSample) {
        if input_type == Input::Rate {
            self.rate.update_amount(src_slot, amount);
        }
    }
}

struct Buffers {
    rate: Buffer,
}

impl Buffers {
    fn new(buffer_size: usize) -> Self {
        Self {
            rate: zero_buffer(buffer_size),
        }
    }
}

pub struct SegmentsEnvelope<L: SegmentsEnvelopeLinks = stub::Links> {
    id: ModuleId,
    params: Params,
    channel_params: [ChannelParams; NUM_CHANNELS],
    buffers: Buffers,
    audio_end: L::AudioEnd,
    ui_end: Option<L::UiEnd>,
    inputs: Inputs,
    output_slot: usize,
    voices: VoicesLayout<Voice>,
}

impl<L: SegmentsEnvelopeLinks> SegmentsEnvelope<L> {
    pub fn take_ui_end(&mut self) -> Option<L::UiEnd> {
        self.ui_end.take()
    }

    pub fn new(id: ModuleId, buffer_size: usize) -> Self {
        Self::from_config(
            &SegmentsEnvelopeConfig {
                id,
                ..SegmentsEnvelopeConfig::default()
            },
            buffer_size,
        )
    }

    pub fn from_config(config: &config::SegmentsEnvelopeConfig, buffer_size: usize) -> Self {
        let mut config = config.clone();
        config.sanitize();

        let (audio_end, ui_end) = L::create_link_pair();

        Self {
            id: config.id,
            params: Params::from_config(&config),
            channel_params: array::from_fn(|channel_idx| {
                ChannelParams::from_config(&config, channel_idx)
            }),
            buffers: Buffers::new(buffer_size),
            audio_end,
            ui_end,
            inputs: Inputs::default(),
            output_slot: usize::MAX,
            voices: new_voices_layout(),
        }
    }

    pub fn get_config(&self) -> SegmentsEnvelopeConfig {
        SegmentsEnvelopeConfig {
            id: self.id,
            steal_level: self.params.steal_level,
            keep_voice_alive: self.params.keep_voice_alive,
            timebase: self.params.timebase,
            rate: get_smoothed_param!(self, rate),
            segments: self.params.segments.clone(),
        }
    }

    set_mono_param!(set_steal_level, steal_level, bool);
    set_mono_param!(set_keep_voice_alive, keep_voice_alive, bool);
    set_mono_param!(set_timebase, timebase, Timebase);

    set_smoothed_param!(set_rate, rate, sanitize(*rate, MIN_RATE, MAX_RATE));

    pub fn set_segments(&mut self, mut segments: Segments) {
        segments.sanitize();
        self.params.segments = segments;
    }

    fn end_level(&self) -> Smp {
        self.params.segments.value_at(1.0)
    }

    fn curve_start(&self) -> Smp {
        let segments = &self.params.segments;

        if segments.num_segments() == 0 {
            0.0
        } else {
            segments.segment(0).y
        }
    }

    /// Renders `voice` into `out`. Phase increments are already in `buffers.rate`.
    fn render_shot(&self, voice: &Voice, out: &mut [Smp], samples: usize) -> Voice {
        match voice.shot {
            Shot::Done => {
                let level = self.end_level();
                out.fill(level);

                Voice {
                    shot: Shot::Done,
                    start_level: voice.start_level,
                    level,
                }
            }
            Shot::Play(phase) => {
                let segments = &self.params.segments;
                let steps = &self.buffers.rate[..samples];
                let phase = segments.render_span_from(out, phase, steps, voice.start_level);
                let written = samples.min(out.len());
                let level = out[..written].last().copied().unwrap_or(voice.level);
                let shot = if phase >= 1.0 {
                    Shot::Done
                } else {
                    Shot::Play(phase)
                };

                Voice {
                    shot,
                    start_level: voice.start_level,
                    level,
                }
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
        let sample_rate = router.sample_rate();
        let tempo = router.tempo();

        router.param(
            &self.inputs.rate,
            &self.channel_params[channel_idx].rate,
            &mut self.buffers.rate,
        );

        let samples = voice_output.output().len();

        self.params.timebase.map_phase_inc(
            &mut self.buffers.rate[..samples],
            tempo,
            sample_rate.recip(),
        );

        let voice = *self.voices.at(channel_idx, voice_idx);
        let voice = self.render_shot(&voice, voice_output.output(), samples);

        if router.need_update_ui_mono() {
            self.audio_end.update_phase(SegmentsEnvelopePhase {
                phase: voice.shot.phase(),
                start_level: voice.start_level,
            });
        }

        *self.voices.at_mut(channel_idx, voice_idx) = voice;
    }

    pub(crate) fn process(&mut self, ctx: &mut ProcessContext<L::EngineEnd>) {
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

impl<L: SegmentsEnvelopeLinks> SynthModule for SegmentsEnvelope<L> {
    fn id(&self) -> ModuleId {
        self.id
    }

    fn inputs(&self) -> &'static [InputMeta] {
        static INPUTS: &[InputMeta] = &[InputMeta::control(Input::Rate)];

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
            let VoiceEvent::Reset {
                voice_idx,
                replaced_voice_idx,
                ..
            } = event
            else {
                continue;
            };

            for channel_idx in 0..NUM_CHANNELS {
                let curve_start = self.curve_start();
                let start_level = if let Some(replaced_voice_idx) = replaced_voice_idx
                    && self.params.steal_level
                {
                    sanitize(
                        self.voices.at(channel_idx, *replaced_voice_idx).level,
                        0.0,
                        1.0,
                    )
                } else {
                    curve_start
                };
                let voice = self.voices.at_mut(channel_idx, *voice_idx);

                voice.shot = Shot::Play(0.0);
                voice.start_level = start_level;
            }
        }
    }

    fn poll_decaying_voices(&self, decaying_voices: &mut [DecayingVoice]) {
        if !self.params.keep_voice_alive {
            return;
        }

        for decaying in decaying_voices.iter_mut().filter(|d| d.is_done()) {
            for voice in self.voices.channels_at(decaying.index()) {
                if matches!(voice.shot, Shot::Play(_)) {
                    decaying.mark_active();
                }
            }
        }
    }

    fn process_ui_events(&mut self) {
        while let Some(event) = self.audio_end.pop_event() {
            match event {
                UiEvent::InputParam { input, value } => {
                    if input == Input::Rate {
                        self.set_rate(value);
                    }
                }
                UiEvent::SegmentsUpdated => {
                    let segments = self.audio_end.read_segments().clone();
                    self.set_segments(segments);
                }
                UiEvent::StealLevel(value) => self.set_steal_level(value),
                UiEvent::KeepVoiceAlive(value) => self.set_keep_voice_alive(value),
                UiEvent::Timebase(timebase) => self.set_timebase(timebase),
            }
        }
    }
}
