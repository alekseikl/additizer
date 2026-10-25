use serde::{Deserialize, Serialize};

use addi_dsp::{segments::Segments, utils::sanitize};

use super::Timebase;
use crate::synth_engine::{ModuleId, Smp, StereoSample};

pub(super) const MIN_RATE: Smp = 0.0;
pub(super) const MAX_RATE: Smp = 50.0;
pub(super) const MIN_PHASE_SHIFT: Smp = 0.0;
pub(super) const MAX_PHASE_SHIFT: Smp = 1.0;

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
    /// Start offset on the curve, from 0 to 1. Applied once, when a voice is triggered.
    #[serde(default)]
    pub phase_shift: StereoSample,
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
            phase_shift: 0.0.into(),
            timebase: Timebase::Seconds,
            segments: Segments::default(),
        }
    }
}

impl SegmentsEnvelopeConfig {
    /// Clamp the length, phase shift, and the breakpoint curve.
    pub fn sanitize(&mut self) {
        self.rate = self.rate.map(|rate| sanitize(rate, MIN_RATE, MAX_RATE));
        self.phase_shift = self
            .phase_shift
            .map(|phase_shift| sanitize(phase_shift, MIN_PHASE_SHIFT, MAX_PHASE_SHIFT));
        self.segments.sanitize();
    }
}
