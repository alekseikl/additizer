use serde::{Deserialize, Serialize};

use addi_dsp::{segments::Segments, utils::sanitize};

use super::Timebase;
use crate::synth_engine::{ModuleId, Smp, StereoSample};

pub(super) const MIN_RATE: Smp = -100.0;
pub(super) const MAX_RATE: Smp = 100.0;

fn default_rate() -> StereoSample {
    1.0.into()
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SegmentsEnvelopeConfig {
    pub id: ModuleId,
    /// A stolen note leaves the first segment from the replaced voice's level.
    #[serde(default, alias = "steal_phase")]
    pub steal_level: bool,
    #[serde(default)]
    pub keep_voice_alive: bool,
    /// Length of one pass, in [`Timebase`] units. `1.0` lasts one unit.
    #[serde(default = "default_rate")]
    pub rate: StereoSample,
    #[serde(default)]
    pub timebase: Timebase,
    #[serde(default)]
    pub segments: Segments,
}

impl Default for SegmentsEnvelopeConfig {
    fn default() -> Self {
        Self {
            id: -1,
            steal_level: false,
            keep_voice_alive: false,
            rate: 1.0.into(),
            timebase: Timebase::Seconds,
            segments: Segments::default(),
        }
    }
}

impl SegmentsEnvelopeConfig {
    /// Clamp the length and the breakpoint curve.
    pub fn sanitize(&mut self) {
        self.rate = self.rate.map(|rate| sanitize(rate, MIN_RATE, MAX_RATE));
        self.segments.sanitize();
    }
}
