use serde::{Deserialize, Serialize};

use addi_dsp::segments::time_segments::TimeSegments;

use crate::synth_engine::ModuleId;

fn default_segments() -> TimeSegments {
    TimeSegments::default()
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
    /// Breakpoints in seconds. Segment `i` lasts until the next point, plus `TimeMix(i)`.
    #[serde(default = "default_segments")]
    pub segments: TimeSegments,
}

impl Default for MultiEnvelopeConfig {
    fn default() -> Self {
        Self {
            id: -1,
            steal_level: false,
            keep_voice_alive: false,
            segments: default_segments(),
        }
    }
}

impl MultiEnvelopeConfig {
    pub fn sanitize(&mut self) {
        self.segments.sanitize();
    }
}
