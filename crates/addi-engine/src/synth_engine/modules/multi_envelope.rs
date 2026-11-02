use std::array;

mod config;
mod link;
pub mod stub;

#[cfg(test)]
mod tests;

pub use config::MultiEnvelopeConfig;
pub use link::{MultiEnvelopeAudioEnd, MultiEnvelopeLinks, MultiEnvelopeUiEnd, UiEvent, UiUpdate};

use addi_dsp::time_segments::{MAX_SEGMENTS, TimePoint, TimeSegments};

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

struct Params {
    segments: TimeSegments,
    steal_level: bool,
    keep_voice_alive: bool,
}

impl Params {
    fn from_config(c: &config::MultiEnvelopeConfig) -> Self {
        Self {
            segments: c.segments.clone(),
            steal_level: c.steal_level,
            keep_voice_alive: c.keep_voice_alive,
        }
    }
}

/// Playhead published for the editor. `time` is seconds on the drawn curve.
#[derive(Clone, Copy, Debug, Default)]
pub struct MultiEnvelopePhase {
    pub time: StereoSample,
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
    time_mix: [MixedSlots; MAX_SEGMENTS],
}

impl Default for Inputs {
    fn default() -> Self {
        Self {
            time_mix: array::from_fn(|index| MixedSlots::new(Input::TimeMix(index as u8))),
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

            if let Input::TimeMix(index) = input.input_type
                && (index as usize) < MAX_SEGMENTS
            {
                result.time_mix[index as usize] = input.clone();
            }
        }

        result
    }

    fn update_amount(&mut self, input_type: Input, src_slot: usize, amount: StereoSample) {
        if let Input::TimeMix(index) = input_type
            && (index as usize) < MAX_SEGMENTS
        {
            self.time_mix[index as usize].update_amount(src_slot, amount);
        }
    }
}

pub struct MultiEnvelope<L: MultiEnvelopeLinks = stub::Links> {
    id: ModuleId,
    params: Params,
    audio_end: L::AudioEnd,
    ui_end: Option<L::UiEnd>,
    inputs: Inputs,
    output_slot: usize,
    voices: VoicesLayout<Voice>,
    playhead: MultiEnvelopePhase,
    inputs_meta: Vec<InputMeta>,
}

impl<L: MultiEnvelopeLinks> MultiEnvelope<L> {
    pub fn take_ui_end(&mut self) -> Option<L::UiEnd> {
        self.ui_end.take()
    }

    pub fn new(id: ModuleId) -> Self {
        Self::from_config(&MultiEnvelopeConfig {
            id,
            ..MultiEnvelopeConfig::default()
        })
    }

    pub fn from_config(config: &config::MultiEnvelopeConfig) -> Self {
        let mut config = config.clone();
        config.sanitize();

        let (audio_end, ui_end) = L::create_link_pair();

        let mut envelope = Self {
            id: config.id,
            params: Params::from_config(&config),
            audio_end,
            ui_end,
            inputs: Inputs::default(),
            output_slot: usize::MAX,
            voices: new_voices_layout(),
            playhead: MultiEnvelopePhase::default(),
            inputs_meta: Vec::with_capacity(MAX_SEGMENTS),
        };

        envelope.build_inputs_meta();
        envelope
    }

    pub fn get_config(&self) -> MultiEnvelopeConfig {
        MultiEnvelopeConfig {
            id: self.id,
            steal_level: self.params.steal_level,
            keep_voice_alive: self.params.keep_voice_alive,
            segments: self.params.segments.clone(),
        }
    }

    set_mono_param!(set_steal_level, steal_level, bool);
    set_mono_param!(set_keep_voice_alive, keep_voice_alive, bool);

    pub fn set_segments(&mut self, mut segments: TimeSegments) {
        segments.sanitize();

        let changed = segments.num_segments() != self.params.segments.num_segments();

        self.params.segments = segments;

        if changed {
            self.build_inputs_meta();
            self.audio_end.refresh_routing();
        }
    }

    fn build_inputs_meta(&mut self) {
        self.inputs_meta.clear();

        for index in 0..self.params.segments.num_segments() {
            self.inputs_meta
                .push(InputMeta::control(Input::TimeMix(index as u8)));
        }
    }

    fn playhead_time(segments: &TimeSegments, stage: Stage) -> Smp {
        match stage {
            Stage::Sustain => segments
                .release_point()
                .map(|point| point.time)
                .unwrap_or_else(|| segments.total_time()),
            Stage::Trigger(_) | Stage::Done => segments.total_time(),
            Stage::Segment {
                index, position, ..
            } => segments.time_at(index, position),
        }
    }

    fn playhead_level(segments: &TimeSegments, voice: &Voice) -> Smp {
        match voice.stage {
            Stage::Sustain => Self::sustain_level(segments, voice.last_level),
            Stage::Trigger(_) | Stage::Done => Self::rest_level(segments, voice),
            Stage::Segment {
                index, position, ..
            } => {
                let start = Self::segment_start(segments, voice, index);
                let end = segments.point(index + 1);

                TimePoint::level_between(start, end, position)
            }
        }
    }

    fn segment_start(segments: &TimeSegments, voice: &Voice, index: usize) -> TimePoint {
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

    /// Samples in one pass of the loop at the current segment durations.
    fn loop_samples(segments: &TimeSegments, durations: &[Smp], sample_rate: Smp) -> usize {
        let Some(start) = segments.loop_point_idx() else {
            return 0;
        };

        let end = segments
            .release_point_idx()
            .unwrap_or(segments.num_points() - 1);
        let seconds: Smp = (start..end).map(|index| durations[index]).sum();

        (seconds * sample_rate) as usize
    }

    /// Stage after segment `index` finishes.
    fn next_stage(
        segments: &TimeSegments,
        durations: &[Smp],
        index: usize,
        note_on: bool,
        sample_rate: Smp,
    ) -> Stage {
        let next = index + 1;
        let spans = segments.num_segments();
        let looping = Self::loop_samples(segments, durations, sample_rate) >= MIN_LOOP_SAMPLES;

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

    fn sustain_level(segments: &TimeSegments, fallback: Smp) -> Smp {
        segments
            .release_point()
            .map(|point| point.y)
            .unwrap_or(fallback)
    }

    fn rest_level(segments: &TimeSegments, voice: &Voice) -> Smp {
        let last = segments.num_points() - 1;

        if segments.release_point_idx() == Some(last)
            && let Some(level) = voice.release.level()
        {
            return level;
        }

        segments.point(last).y
    }

    fn begin_release(segments: &TimeSegments, voice: &mut Voice, level: Smp) {
        let Some(index) = segments.release_point_idx() else {
            voice.release = Release::None;

            if matches!(voice.stage, Stage::Sustain) {
                voice.stage = Stage::Done;
            }

            return;
        };

        voice.release = Release::Releasing(level);

        if index >= segments.num_segments() {
            voice.stage = Stage::Done;
            voice.last_level = level;
        } else {
            voice.stage = Stage::segment(index, 0.0);
        }
    }

    fn render_region(
        segments: &TimeSegments,
        durations: &[Smp],
        voice: &mut Voice,
        out: &mut [Smp],
        seconds_per_sample: Smp,
        sample_rate: Smp,
    ) {
        let len = out.len();
        let mut pos = 0;
        let mut hops = 0;

        while pos < len {
            hops += 1;

            if hops > MAX_SEGMENTS + 2 {
                out[pos..].fill(Self::rest_level(segments, voice));
                voice.stage = Stage::Done;
                break;
            }

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
                    let duration = durations[index];

                    match TimePoint::render_segment(
                        &mut out[pos..],
                        position,
                        seconds_per_sample,
                        start,
                        end,
                        duration,
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
                                durations,
                                index,
                                voice.release.note_on(),
                                sample_rate,
                            );
                            continue;
                        }
                    }
                }
            }

            break;
        }
    }

    fn apply_trigger(segments: &TimeSegments, voice: &mut Voice, level: Option<Smp>) {
        voice.stage = Stage::Segment {
            index: 0,
            position: 0.0,
            start: level,
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
        let seconds_per_sample = sample_rate.recip();
        let segments = &self.params.segments;
        let mut durations = [0.0; MAX_SEGMENTS];

        for (index, duration) in durations
            .iter_mut()
            .enumerate()
            .take(segments.num_segments())
        {
            *duration = router
                .scalar(&self.inputs.time_mix[index], segments.duration(index))
                .max(0.0);
        }

        let voice = self.voices.at_mut(channel_idx, voice_idx);
        let trigger = match voice.stage {
            Stage::Trigger(level) => Some(level),
            _ => None,
        };

        if let Some(level) = trigger {
            Self::apply_trigger(segments, voice, level);
        }

        let release_at = match voice.release {
            Release::Pending(offset) => Some(router.block_to_voice_offset(offset)),
            Release::None | Release::Releasing(_) => None,
        };
        let out = voice_output.output();
        let len = out.len();
        let mut pos = 0;
        let mut release_at = release_at.map(|at| at.min(len));

        while pos < len {
            let end = release_at.unwrap_or(len).max(pos);

            Self::render_region(
                segments,
                &durations,
                voice,
                &mut out[pos..end],
                seconds_per_sample,
                sample_rate,
            );

            pos = end;

            if release_at.take().is_some() {
                let level = out[..pos].last().copied().unwrap_or(voice.last_level);
                Self::begin_release(segments, voice, level);
            }
        }

        voice.last_level = out.last().copied().unwrap_or_default();

        if router.need_update_ui() {
            self.playhead.time[channel_idx] = Self::playhead_time(segments, voice.stage);
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

impl<L: MultiEnvelopeLinks> SynthModule for MultiEnvelope<L> {
    fn id(&self) -> ModuleId {
        self.id
    }

    fn inputs(&self) -> &[InputMeta] {
        &self.inputs_meta
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
        if !self.params.keep_voice_alive || self.params.segments.release_point_idx().is_none() {
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
                UiEvent::SegmentsUpdated => {
                    let segments = self.audio_end.read_segments().clone();
                    self.set_segments(segments);
                }
                UiEvent::StealLevel(value) => self.set_steal_level(value),
                UiEvent::KeepVoiceAlive(value) => self.set_keep_voice_alive(value),
            }
        }
    }
}
