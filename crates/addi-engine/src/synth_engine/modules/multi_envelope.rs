use std::array;

mod config;
mod link;
pub mod stub;

#[cfg(test)]
mod tests;

pub use config::{MAX_DURATION_OFFSET, MAX_LEVEL_OFFSET, MultiEnvelopeConfig};
pub use link::{MultiEnvelopeAudioEnd, MultiEnvelopeLinks, MultiEnvelopeUiEnd, UiEvent, UiUpdate};

use addi_dsp::segments::{
    render_segment,
    time_segments::{MAX_SEGMENTS, TimePoint, TimeSegments},
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

#[derive(Clone, Copy, Default)]
struct SegmentParams {
    duration: Smp,
    level: Smp,
}

struct ChannelParams {
    segments: [SegmentParams; MAX_SEGMENTS],
}

impl ChannelParams {
    fn from_config(c: &config::MultiEnvelopeConfig, channel_idx: usize) -> Self {
        Self {
            segments: array::from_fn(|index| SegmentParams {
                duration: c.duration_offset(index)[channel_idx],
                level: c.level_offset(index)[channel_idx],
            }),
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

#[derive(Debug)]
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

struct SegmentInput {
    duration: MixedSlots,
    level: MixedSlots,
}

pub struct Inputs {
    segments: [SegmentInput; MAX_SEGMENTS],
}

impl Default for Inputs {
    fn default() -> Self {
        Self {
            segments: array::from_fn(|index| SegmentInput {
                duration: MixedSlots::new(Input::DurationMix(index as u8)),
                level: MixedSlots::new(Input::LinLevelMix(index as u8)),
            }),
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
                Input::DurationMix(index) if (index as usize) < MAX_SEGMENTS => {
                    result.segments[index as usize].duration = input.clone();
                }
                Input::LinLevelMix(index) if (index as usize) < MAX_SEGMENTS => {
                    result.segments[index as usize].level = input.clone();
                }
                _ => {}
            }
        }

        result
    }

    fn update_amount(&mut self, input_type: Input, src_slot: usize, amount: StereoSample) {
        match input_type {
            Input::DurationMix(index) if (index as usize) < MAX_SEGMENTS => {
                self.segments[index as usize]
                    .duration
                    .update_amount(src_slot, amount);
            }
            Input::LinLevelMix(index) if (index as usize) < MAX_SEGMENTS => {
                self.segments[index as usize]
                    .level
                    .update_amount(src_slot, amount);
            }
            _ => {}
        }
    }
}

pub struct MultiEnvelope<L: MultiEnvelopeLinks = stub::Links> {
    id: ModuleId,
    params: Params,
    channel_params: [ChannelParams; NUM_CHANNELS],
    audio_end: L::AudioEnd,
    ui_end: Option<L::UiEnd>,
    inputs: Inputs,
    output_slot: usize,
    voices: VoicesLayout<Voice>,
    modulated: [SegmentParams; MAX_SEGMENTS],
    inputs_meta: Vec<InputMeta>,
}

struct Ctx<'a> {
    segments: &'a TimeSegments,
    modulated: &'a [SegmentParams; MAX_SEGMENTS],
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
            channel_params: array::from_fn(|channel_idx| {
                ChannelParams::from_config(&config, channel_idx)
            }),
            audio_end,
            ui_end,
            inputs: Inputs::default(),
            output_slot: usize::MAX,
            voices: new_voices_layout(),
            modulated: [SegmentParams::default(); MAX_SEGMENTS],
            inputs_meta: Vec::with_capacity(MAX_SEGMENTS * 2),
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
            offsets: self.offset_config(),
        }
    }

    fn offset_config(&self) -> Vec<config::SegmentOffset> {
        let mut offsets: Vec<_> = (0..MAX_SEGMENTS)
            .map(|index| config::SegmentOffset {
                duration: StereoSample::from_iter(
                    self.channel_params
                        .iter()
                        .map(|channel| channel.segments[index].duration),
                ),
                level: StereoSample::from_iter(
                    self.channel_params
                        .iter()
                        .map(|channel| channel.segments[index].level),
                ),
            })
            .collect();

        while offsets.last().is_some_and(|offset| {
            offset.duration == StereoSample::ZERO && offset.level == StereoSample::ZERO
        }) {
            offsets.pop();
        }

        offsets
    }

    set_mono_param!(set_steal_level, steal_level, bool);
    set_mono_param!(set_keep_voice_alive, keep_voice_alive, bool);

    pub fn set_duration_offset(&mut self, index: u8, value: StereoSample) {
        let index = index as usize;

        if index >= MAX_SEGMENTS {
            return;
        }

        let value = value.clamp(-MAX_DURATION_OFFSET, MAX_DURATION_OFFSET);

        for (channel, value) in self.channel_params.iter_mut().zip(value.iter()) {
            channel.segments[index].duration = *value;
        }
    }

    pub fn set_level_offset(&mut self, index: u8, value: StereoSample) {
        let index = index as usize;

        if index >= MAX_SEGMENTS {
            return;
        }

        let value = value.clamp(-MAX_LEVEL_OFFSET, MAX_LEVEL_OFFSET);

        for (channel, value) in self.channel_params.iter_mut().zip(value.iter()) {
            channel.segments[index].level = *value;
        }
    }

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
            let index = index as u8;

            self.inputs_meta
                .push(InputMeta::control(Input::DurationMix(index)));
            self.inputs_meta
                .push(InputMeta::control(Input::LinLevelMix(index)));
        }
    }

    fn playhead_time(ctx: &Ctx, voice: &Voice) -> Smp {
        match voice.stage {
            Stage::Sustain => ctx
                .segments
                .release_point_idx()
                .map(|index| ctx.segments.time(index))
                .unwrap_or_else(|| ctx.segments.total_time()),
            Stage::Trigger(_) | Stage::Done => ctx.segments.total_time(),
            Stage::Segment {
                index, position, ..
            } => ctx.segments.time_at(index, position),
        }
    }

    fn level_at(ctx: &Ctx, index: usize) -> Smp {
        ctx.modulated
            .get(index)
            .map(|segment| segment.level)
            .unwrap_or(0.0)
    }

    fn playhead_level(ctx: &Ctx, voice: &Voice) -> Smp {
        match voice.stage {
            Stage::Sustain => Self::sustain_level(ctx, voice.last_level),
            Stage::Trigger(_) | Stage::Done => Self::rest_level(ctx, voice),
            Stage::Segment {
                index, position, ..
            } => {
                let start = Self::segment_start(ctx, voice, index);
                let end = Self::segment_end(ctx, index);

                TimePoint::level_between(start, end, position)
            }
        }
    }

    /// End point of segment `index`, shifted by that segment's level offset.
    fn segment_end(ctx: &Ctx, index: usize) -> TimePoint {
        let mut end = ctx.segments.point(index + 1);

        end.y += Self::level_at(ctx, index);
        end
    }

    /// Start of segment `index`. A stolen or release level is already an output value,
    /// so the offset is added only to a point taken from the curve.
    fn segment_start(ctx: &Ctx, voice: &Voice, index: usize) -> TimePoint {
        let mut start = ctx.segments.point(index);

        if ctx.segments.release_point_idx() == Some(index)
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
            return start;
        }

        start.y += Self::level_at(ctx, index);
        start
    }

    /// Samples in one pass of the loop at the current segment durations.
    fn loop_samples(ctx: &Ctx, sample_rate: Smp) -> usize {
        let Some(start) = ctx.segments.loop_point_idx() else {
            return 0;
        };

        let end = ctx
            .segments
            .release_point_idx()
            .unwrap_or(ctx.segments.num_points() - 1);
        let seconds: Smp = (start..end)
            .map(|index| ctx.modulated[index].duration)
            .sum();

        (seconds * sample_rate) as usize
    }

    /// Stage after segment `index` finishes.
    fn next_stage(ctx: &Ctx, index: usize, note_on: bool, sample_rate: Smp) -> Stage {
        let next = index + 1;
        let spans = ctx.segments.num_segments();
        let looping = Self::loop_samples(ctx, sample_rate) >= MIN_LOOP_SAMPLES;

        if note_on
            && let Some(release) = ctx.segments.release_point_idx()
            && next >= release
        {
            if looping && let Some(index) = ctx.segments.loop_point_idx() {
                return Stage::segment(index, 0.0);
            }

            return Stage::Sustain;
        }

        if next < spans {
            return Stage::segment(next, 0.0);
        }

        if ctx.segments.release_point_idx().is_none()
            && looping
            && let Some(index) = ctx.segments.loop_point_idx()
        {
            return Stage::segment(index, 0.0);
        }

        Stage::Done
    }

    fn sustain_level(ctx: &Ctx, fallback: Smp) -> Smp {
        let Some(index) = ctx.segments.release_point_idx() else {
            return fallback;
        };

        let offset = index
            .checked_sub(1)
            .map(|index| Self::level_at(ctx, index))
            .unwrap_or(0.0);

        ctx.segments.point(index).y + offset
    }

    fn rest_level(ctx: &Ctx, voice: &Voice) -> Smp {
        let last = ctx.segments.num_points() - 1;

        if ctx.segments.release_point_idx() == Some(last)
            && let Some(level) = voice.release.level()
        {
            return level;
        }

        ctx.segments.point(last).y + Self::level_at(ctx, last - 1)
    }

    fn begin_release(ctx: &Ctx, voice: &mut Voice, level: Smp) {
        let Some(index) = ctx.segments.release_point_idx() else {
            voice.release = Release::None;

            if matches!(voice.stage, Stage::Sustain) {
                voice.stage = Stage::Done;
            }

            return;
        };

        let num_segments = ctx.segments.num_segments();

        voice.release = Release::Releasing(level);

        if index >= num_segments {
            voice.stage = Stage::Done;
            voice.last_level = level;
        } else {
            voice.stage = Stage::segment(index, 0.0);
        }
    }

    fn render_region(
        ctx: &Ctx,
        voice: &mut Voice,
        out: &mut [Smp],
        seconds_per_sample: Smp,
        sample_rate: Smp,
    ) {
        let len = out.len();
        let mut pos = 0;

        while pos < len {
            match voice.stage {
                Stage::Sustain => {
                    let last_level = voice.last_level;

                    out[pos..].fill(Self::sustain_level(ctx, last_level));
                }
                Stage::Done => {
                    out[pos..].fill(Self::rest_level(ctx, voice));
                }
                Stage::Trigger(_) => unreachable!("trigger is applied before render"),
                Stage::Segment {
                    index, position, ..
                } => {
                    let start = Self::segment_start(ctx, voice, index);
                    let end = Self::segment_end(ctx, index);
                    let duration = ctx.modulated[index].duration;
                    let mut result = Err(0);

                    if duration > 0.0 {
                        result = render_segment(
                            &mut out[pos..],
                            position,
                            seconds_per_sample / duration,
                            start.y..end.y,
                            start.curve,
                        );
                    }

                    match result {
                        Ok(position) => {
                            if let Stage::Segment { position: at, .. } = &mut voice.stage {
                                *at = position;
                            }
                        }
                        Err(written) => {
                            pos += written;

                            let note_on = voice.release.note_on();
                            voice.stage = Self::next_stage(ctx, index, note_on, sample_rate);
                            continue;
                        }
                    }
                }
            }

            break;
        }
    }

    fn apply_trigger(ctx: &Ctx, voice: &mut Voice, level: Option<Smp>) {
        voice.stage = Stage::Segment {
            index: 0,
            position: 0.0,
            start: level,
        };
        voice.last_level = Self::playhead_level(ctx, voice);
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
        let channel = &self.channel_params[channel_idx];

        for index in 0..segments.num_segments() {
            let input = &self.inputs.segments[index];
            let offset = &channel.segments[index];
            let duration_offset = router.scalar(&input.duration, offset.duration);

            self.modulated[index] = SegmentParams {
                duration: (segments.duration(index) + duration_offset).max(0.0),
                level: router.scalar(&input.level, offset.level).clamp(0.0, 1.0),
            };
        }

        let ctx = Ctx {
            segments: &self.params.segments,
            modulated: &self.modulated,
        };
        let voice = self.voices.at_mut(channel_idx, voice_idx);
        let trigger = match voice.stage {
            Stage::Trigger(level) => Some(level),
            _ => None,
        };

        if let Some(level) = trigger {
            Self::apply_trigger(&ctx, voice, level);
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
                &ctx,
                voice,
                &mut out[pos..end],
                seconds_per_sample,
                sample_rate,
            );

            pos = end;

            if release_at.take().is_some() {
                let last_level = voice.last_level;
                let level = out[..pos].last().copied().unwrap_or(last_level);

                Self::begin_release(&ctx, voice, level);
            }
        }

        voice.last_level = out.last().copied().unwrap_or_default();

        if router.need_update_ui() {
            let time = Self::playhead_time(&ctx, voice);
            let level = Self::playhead_level(&ctx, voice);

            self.audio_end.update_phase(channel_idx, time, level);
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
                UiEvent::InputParam { input, value } => match input {
                    Input::DurationMix(index) => self.set_duration_offset(index, value),
                    Input::LinLevelMix(index) => self.set_level_offset(index, value),
                    _ => {}
                },
                UiEvent::StealLevel(value) => self.set_steal_level(value),
                UiEvent::KeepVoiceAlive(value) => self.set_keep_voice_alive(value),
            }
        }
    }
}
