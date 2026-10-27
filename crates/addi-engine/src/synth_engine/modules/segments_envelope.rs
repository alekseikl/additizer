use std::array;

mod config;
mod link;
pub mod stub;
mod timebase;

#[cfg(test)]
mod tests;

pub use config::SegmentsEnvelopeConfig;
use config::{MAX_PHASE_SHIFT, MAX_RATE, MIN_PHASE_SHIFT, MIN_RATE};
pub use link::{SegmentsEnvelopeAudioEnd, SegmentsEnvelopeLinks, SegmentsEnvelopeUiEnd, UiEvent};
pub use timebase::Timebase;

use addi_dsp::{
    segments::{SegPoint, Segments},
    utils::sanitize,
};

use crate::synth_engine::{
    Input, ModuleId, Smp, StereoSample,
    buffer::{VoicesLayout, new_voices_layout},
    routing::{
        ControlRouterType, DataType, InputMeta, InputSlots, MixedSlots, ProcessContext,
        RouterFactory, SamplesOutput, VoiceEvent, VoiceTarget,
    },
    synth_module::SynthModule,
    voices::NUM_CHANNELS,
    voices_handler::DecayingVoice,
};

const MIN_LOOP_SAMPLES: usize = 8;

struct ChannelParams {
    rate: Smp,
    phase_shift: Smp,
}

impl ChannelParams {
    fn from_config(c: &config::SegmentsEnvelopeConfig, channel_idx: usize) -> Self {
        Self {
            rate: c.rate[channel_idx],
            phase_shift: c.phase_shift[channel_idx],
        }
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

/// Playhead published for the editor. Each channel is a point on the curve.
#[derive(Clone, Copy, Debug, Default)]
pub struct SegmentsEnvelopePhase {
    pub phase: StereoSample,
    pub level: StereoSample,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Stage {
    /// `Some` is the stolen opening level.
    Trigger(Option<Smp>),
    /// `start` replaces the first point during the first pass of segment 0.
    Segment {
        index: usize,
        position: Smp,
        start: Option<Smp>,
    },
    Sustain,
    Done,
}

impl Stage {
    fn segment(index: usize, position: Smp) -> Self {
        Self::Segment {
            index,
            position,
            start: None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Release {
    None,
    Pending(usize),
    Releasing(Smp),
}

impl Release {
    fn level(self) -> Option<Smp> {
        match self {
            Self::Releasing(level) => Some(level),
            Self::None | Self::Pending(_) => None,
        }
    }

    fn note_on(self) -> bool {
        !matches!(self, Self::Releasing(_))
    }
}

#[derive(Clone, Copy, Debug)]
struct Voice {
    stage: Stage,
    /// Last sample this voice wrote. A stolen note copies it.
    last_level: Smp,
    release: Release,
}

impl Default for Voice {
    fn default() -> Self {
        Self {
            stage: Stage::Done,
            last_level: 0.0,
            release: Release::None,
        }
    }
}

pub struct Inputs {
    rate: MixedSlots,
    phase_shift: MixedSlots,
}

impl Default for Inputs {
    fn default() -> Self {
        Self {
            rate: MixedSlots::new(Input::Rate),
            phase_shift: MixedSlots::new(Input::PhaseShift),
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
                Input::Rate => result.rate = input.clone(),
                Input::PhaseShift => result.phase_shift = input.clone(),
                _ => (),
            }
        }

        result
    }

    fn update_amount(&mut self, input_type: Input, src_slot: usize, amount: StereoSample) {
        match input_type {
            Input::Rate => self.rate.update_amount(src_slot, amount),
            Input::PhaseShift => self.phase_shift.update_amount(src_slot, amount),
            _ => (),
        }
    }
}

pub struct SegmentsEnvelope<L: SegmentsEnvelopeLinks = stub::Links> {
    id: ModuleId,
    params: Params,
    channel_params: [ChannelParams; NUM_CHANNELS],
    audio_end: L::AudioEnd,
    ui_end: Option<L::UiEnd>,
    inputs: Inputs,
    output_slot: usize,
    voices: VoicesLayout<Voice>,
    playhead: SegmentsEnvelopePhase,
}

impl<L: SegmentsEnvelopeLinks> SegmentsEnvelope<L> {
    pub fn take_ui_end(&mut self) -> Option<L::UiEnd> {
        self.ui_end.take()
    }

    pub fn new(id: ModuleId) -> Self {
        Self::from_config(&SegmentsEnvelopeConfig {
            id,
            ..SegmentsEnvelopeConfig::default()
        })
    }

    pub fn from_config(config: &config::SegmentsEnvelopeConfig) -> Self {
        let mut config = config.clone();
        config.sanitize();

        let (audio_end, ui_end) = L::create_link_pair();

        Self {
            id: config.id,
            params: Params::from_config(&config),
            channel_params: array::from_fn(|channel_idx| {
                ChannelParams::from_config(&config, channel_idx)
            }),
            audio_end,
            ui_end,
            inputs: Inputs::default(),
            output_slot: usize::MAX,
            voices: new_voices_layout(),
            playhead: SegmentsEnvelopePhase::default(),
        }
    }

    pub fn get_config(&self) -> SegmentsEnvelopeConfig {
        SegmentsEnvelopeConfig {
            id: self.id,
            steal_level: self.params.steal_level,
            keep_voice_alive: self.params.keep_voice_alive,
            timebase: self.params.timebase,
            rate: get_stereo_param!(self, rate),
            phase_shift: get_stereo_param!(self, phase_shift),
            segments: self.params.segments.clone(),
        }
    }

    set_mono_param!(set_steal_level, steal_level, bool);
    set_mono_param!(set_keep_voice_alive, keep_voice_alive, bool);
    set_mono_param!(set_timebase, timebase, Timebase);

    set_stereo_param!(set_rate, rate, sanitize(*rate, MIN_RATE, MAX_RATE));
    set_stereo_param!(
        set_phase_shift,
        phase_shift,
        sanitize(*phase_shift, MIN_PHASE_SHIFT, MAX_PHASE_SHIFT)
    );

    pub fn set_segments(&mut self, mut segments: Segments) {
        segments.sanitize();
        self.params.segments = segments;
    }

    fn playhead_phase(segments: &Segments, stage: Stage) -> Smp {
        match stage {
            Stage::Sustain => segments.release_point().map(|point| point.x).unwrap_or(1.0),
            Stage::Trigger(_) | Stage::Done => 1.0,
            Stage::Segment {
                index, position, ..
            } => segments.phase_at(index, position),
        }
    }

    fn playhead_level(segments: &Segments, voice: &Voice) -> Smp {
        let start_level = match voice.stage {
            Stage::Segment {
                index: 0, start, ..
            } => start,
            _ => None,
        };

        segments.value_at(
            Self::playhead_phase(segments, voice.stage),
            start_level,
            voice.release.level(),
        )
    }

    fn segment_start(segments: &Segments, voice: &Voice, index: usize) -> SegPoint {
        let mut start = segments.point(index);

        if segments.release_point_idx() == Some(index)
            && let Some(level) = voice.release.level()
        {
            start.y = level;
            return start;
        }

        if index == 0
            && let Stage::Segment {
                start: Some(level), ..
            } = voice.stage
        {
            start.y = level;
        }

        start
    }

    /// Stage after segment `index` finishes.
    fn next_stage(segments: &Segments, index: usize, note_on: bool, phase_step: Smp) -> Stage {
        let next = index + 1;
        let spans = segments.num_points() - 1;
        let looping = segments.loop_samples(phase_step) >= MIN_LOOP_SAMPLES;

        if note_on
            && let Some(release) = segments.release_point_idx()
            && next >= release
        {
            if looping && let Some(index) = segments.loop_point_idx() {
                return Stage::segment(index, 0.0);
            }

            return Stage::Sustain;
        }

        if next < spans {
            return Stage::segment(next, 0.0);
        }

        if segments.release_point_idx().is_none()
            && looping
            && let Some(index) = segments.loop_point_idx()
        {
            return Stage::segment(index, 0.0);
        }

        Stage::Done
    }

    fn sustain_level(segments: &Segments, fallback: Smp) -> Smp {
        segments
            .release_point()
            .map(|point| point.y)
            .unwrap_or(fallback)
    }

    fn rest_level(segments: &Segments, voice: &Voice) -> Smp {
        let last = segments.num_points() - 1;

        if segments.release_point_idx() == Some(last)
            && let Some(level) = voice.release.level()
        {
            return level;
        }

        segments.point(last).y
    }

    fn begin_release(segments: &Segments, voice: &mut Voice, level: Smp) {
        let Some(index) = segments.release_point_idx() else {
            voice.release = Release::None;

            if matches!(voice.stage, Stage::Sustain) {
                voice.stage = Stage::Done;
            }

            return;
        };

        voice.release = Release::Releasing(level);

        let spans = segments.num_points() - 1;

        if index >= spans {
            voice.stage = Stage::Done;
            voice.last_level = level;
        } else {
            voice.stage = Stage::segment(index, 0.0);
        }
    }

    fn render_region(segments: &Segments, voice: &mut Voice, out: &mut [Smp], phase_step: Smp) {
        let len = out.len();
        let mut pos = 0;

        while pos < len {
            match voice.stage {
                Stage::Sustain => {
                    out[pos..].fill(Self::sustain_level(segments, voice.last_level));
                }
                Stage::Done => {
                    out[pos..].fill(Self::rest_level(segments, voice));
                }
                Stage::Trigger(_) => unreachable!("trigger is applied before render"),
                Stage::Segment {
                    index, position, ..
                } => {
                    let start = Self::segment_start(segments, voice, index);
                    let end = segments.point(index + 1);

                    match SegPoint::render_segment(
                        &mut out[pos..],
                        position,
                        phase_step,
                        start,
                        end,
                    ) {
                        Ok(position) => {
                            if let Stage::Segment { position: at, .. } = &mut voice.stage {
                                *at = position;
                            }
                        }
                        Err(written) => {
                            pos += written;
                            voice.stage = Self::next_stage(
                                segments,
                                index,
                                voice.release.note_on(),
                                phase_step,
                            );
                            continue;
                        }
                    }
                }
            }

            break;
        }
    }

    fn apply_trigger(segments: &Segments, voice: &mut Voice, phase: Smp, level: Option<Smp>) {
        let segment = |index, position| Stage::Segment {
            index,
            position,
            start: if index == 0 { level } else { None },
        };

        voice.stage = if let Some(release) = segments.release_point()
            && phase >= release.x
        {
            match segments.loop_point_idx() {
                Some(index) => segment(index, 0.0),
                None => Stage::Sustain,
            }
        } else {
            let spans = segments.num_points() - 1;
            let mut index = 0;
            let mut position = 0.0;

            for i in 0..spans {
                let start = segments.point(i).x;
                let end = segments.point(i + 1).x;
                let width = end - start;

                if width > 0.0 && phase >= start && phase < end {
                    index = i;
                    position = (phase - start) / width;
                    break;
                }
            }

            segment(index, position)
        };
        voice.last_level = Self::playhead_level(segments, voice);
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
        let rate = router.scalar(&self.inputs.rate, self.channel_params[channel_idx].rate);
        let phase_step = self
            .params
            .timebase
            .phase_inc(rate, tempo, sample_rate.recip());

        let voice = self.voices.at_mut(channel_idx, voice_idx);
        let trigger = match voice.stage {
            Stage::Trigger(level) => Some(level),
            _ => None,
        };

        if let Some(level) = trigger {
            let phase_shift = router
                .scalar(
                    &self.inputs.phase_shift,
                    self.channel_params[channel_idx].phase_shift,
                )
                .rem_euclid(1.0);

            Self::apply_trigger(&self.params.segments, voice, phase_shift, level);
        }

        let release_at = match voice.release {
            Release::Pending(offset) => Some(router.block_to_voice_offset(offset)),
            Release::None | Release::Releasing(_) => None,
        };
        let segments = &self.params.segments;
        let out = voice_output.output();
        let len = out.len();
        let mut pos = 0;
        let mut release_at = release_at.map(|at| at.min(len));

        while pos < len {
            let end = release_at.unwrap_or(len).max(pos);

            Self::render_region(segments, voice, &mut out[pos..end], phase_step);

            pos = end;

            if release_at.take().is_some() {
                let level = out[..pos].last().copied().unwrap_or(voice.last_level);
                Self::begin_release(segments, voice, level);
            }
        }

        voice.last_level = out.last().copied().unwrap_or_default();

        if router.need_update_ui() {
            self.playhead.phase[channel_idx] = Self::playhead_phase(segments, voice.stage);
            self.playhead.level[channel_idx] = Self::playhead_level(segments, voice);

            if channel_idx + 1 == NUM_CHANNELS {
                self.audio_end.update_phase(self.playhead);
            }
        }
    }

    pub(crate) fn process(&mut self, ctx: &mut ProcessContext<L::EngineEnd>) {
        ctx.control(self.id, self.output_slot)
            .for_voices(|rf, target, outputs| {
                self.process_voice(target, outputs, rf);
            });
    }
}

impl<L: SegmentsEnvelopeLinks> SynthModule for SegmentsEnvelope<L> {
    fn id(&self) -> ModuleId {
        self.id
    }

    fn inputs(&self) -> &'static [InputMeta] {
        static INPUTS: &[InputMeta] = &[
            InputMeta::control(Input::Rate),
            InputMeta::control(Input::PhaseShift),
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
                VoiceEvent::Release {
                    voice_idx, offset, ..
                } => {
                    for voice in self.voices.channels_at_mut(*voice_idx) {
                        voice.release = Release::Pending(*offset);
                    }
                }
                VoiceEvent::Reset {
                    voice_idx,
                    replaced_voice_idx,
                    ..
                } => {
                    for channel_idx in 0..NUM_CHANNELS {
                        let start_level = replaced_voice_idx
                            .filter(|_| self.params.steal_level)
                            .map(|replaced_voice_idx| {
                                self.voices.at(channel_idx, replaced_voice_idx).last_level
                            });
                        let voice = self.voices.at_mut(channel_idx, *voice_idx);

                        voice.release = Release::None;
                        voice.stage = Stage::Trigger(start_level);
                    }
                }
                VoiceEvent::Update { .. } | VoiceEvent::Kill { .. } => {}
            }
        }
    }

    fn poll_decaying_voices(&self, decaying_voices: &mut [DecayingVoice]) {
        if !self.params.keep_voice_alive {
            return;
        }

        for decaying in decaying_voices.iter_mut().filter(|d| d.is_done()) {
            for voice in self.voices.channels_at(decaying.index()) {
                if !matches!(voice.stage, Stage::Done) {
                    decaying.mark_active();
                }
            }
        }
    }

    fn process_ui_events(&mut self) {
        while let Some(event) = self.audio_end.pop_event() {
            match event {
                UiEvent::InputParam { input, value } => match input {
                    Input::Rate => self.set_rate(value),
                    Input::PhaseShift => self.set_phase_shift(value),
                    _ => (),
                },
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
