use serde::{Deserialize, Serialize};

use addi_dsp::segments::time_segments::{MAX_POINTS, TimeSegments};

use crate::synth_engine::{ModuleId, Smp, StereoSample};

/// Largest duration offset, in seconds. The slider and the stored value share this range.
pub const MAX_DURATION_OFFSET: Smp = 8.0;
/// Largest level offset. Breakpoints stay in `0..=1`; the offset is added, then the level is clamped back.
pub const MAX_LEVEL_OFFSET: Smp = 1.0;

fn default_segments() -> TimeSegments {
    TimeSegments::default()
}

/// Added to one breakpoint. `duration` is seconds until the next point.
/// A duration on the last point is ignored. `level` is added to this point.
#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct PointOffset {
    #[serde(default)]
    pub duration: StereoSample,
    #[serde(default)]
    pub level: StereoSample,
}

impl Default for PointOffset {
    fn default() -> Self {
        Self {
            duration: StereoSample::ZERO,
            level: StereoSample::ZERO,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct MultiEnvelopeConfig {
    pub id: ModuleId,
    /// A stolen note leaves the first segment from the replaced voice's level.
    #[serde(default)]
    pub steal_level: bool,
    /// Holds a released voice until the curve after the release point finishes.
    /// Ignored when no release point is set.
    #[serde(default)]
    pub keep_voice_alive: bool,
    /// Breakpoints in seconds. Point `i` reaches the next point after `duration(i)` plus `offsets[i].duration`.
    #[serde(default = "default_segments")]
    pub segments: TimeSegments,
    /// Per-point duration and level offsets. A missing entry is zero.
    /// A duration on the last point is ignored. Trailing zeros are omitted;
    /// earlier offsets stay when later points go away.
    #[serde(default)]
    pub offsets: Vec<PointOffset>,
}

impl Default for MultiEnvelopeConfig {
    fn default() -> Self {
        Self {
            id: -1,
            steal_level: false,
            keep_voice_alive: false,
            segments: default_segments(),
            offsets: Vec::new(),
        }
    }
}

impl MultiEnvelopeConfig {
    pub fn duration_offset(&self, index: usize) -> StereoSample {
        self.offsets
            .get(index)
            .map(|offset| offset.duration)
            .unwrap_or_default()
    }

    pub fn level_offset(&self, index: usize) -> StereoSample {
        self.offsets
            .get(index)
            .map(|offset| offset.level)
            .unwrap_or_default()
    }

    pub fn set_duration_offset(&mut self, index: usize, value: StereoSample) {
        let Some(offset) = self.offset_mut(index) else {
            return;
        };

        offset.duration = value.clamp(-MAX_DURATION_OFFSET, MAX_DURATION_OFFSET);
    }

    pub fn set_level_offset(&mut self, index: usize, value: StereoSample) {
        let Some(offset) = self.offset_mut(index) else {
            return;
        };

        offset.level = value.clamp(-MAX_LEVEL_OFFSET, MAX_LEVEL_OFFSET);
    }

    fn offset_mut(&mut self, index: usize) -> Option<&mut PointOffset> {
        if index >= MAX_POINTS {
            return None;
        }

        if self.offsets.len() <= index {
            self.offsets.resize(index + 1, PointOffset::default());
        }

        Some(&mut self.offsets[index])
    }

    pub fn sanitize(&mut self) {
        self.segments.sanitize();
        self.offsets.truncate(MAX_POINTS);

        for offset in &mut self.offsets {
            offset.duration = offset
                .duration
                .clamp(-MAX_DURATION_OFFSET, MAX_DURATION_OFFSET);
            offset.level = offset.level.clamp(-MAX_LEVEL_OFFSET, MAX_LEVEL_OFFSET);
        }

        while self.offsets.last().is_some_and(|offset| {
            offset.duration == StereoSample::ZERO && offset.level == StereoSample::ZERO
        }) {
            self.offsets.pop();
        }
    }
}
