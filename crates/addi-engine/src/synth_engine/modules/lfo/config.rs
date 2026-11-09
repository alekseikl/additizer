use serde::{Deserialize, Serialize};

use addi_dsp::{segments::rel_segments::RelSegments, utils::sanitize};

use super::Timebase;
use crate::synth_engine::{ModuleId, Smp, StereoSample};

pub(super) const MIN_RATE: Smp = -100.0;
pub(super) const MAX_RATE: Smp = 100.0;
pub(super) const MIN_PHASE_SHIFT: Smp = -1.0;
pub(super) const MAX_PHASE_SHIFT: Smp = 1.0;

fn default_rate() -> StereoSample {
    1.0.into()
}

#[derive(Clone, Serialize, Deserialize)]
pub struct LfoConfig {
    pub id: ModuleId,
    #[serde(default)]
    pub steal_phase: bool,
    /// Start a new voice at the host timeline phase.
    #[serde(default)]
    pub sync: bool,
    /// Rate in [`Timebase`] units. `1.0` is one cycle per unit.
    #[serde(default = "default_rate", alias = "frequency")]
    pub rate: StereoSample,
    #[serde(default)]
    pub timebase: Timebase,
    #[serde(default)]
    pub phase_shift: StereoSample,
    #[serde(default)]
    pub segments: RelSegments,
}

impl Default for LfoConfig {
    fn default() -> Self {
        Self {
            id: -1,
            steal_phase: false,
            sync: false,
            rate: 1.0.into(),
            timebase: Timebase::Hertz,
            phase_shift: 0.0.into(),
            segments: RelSegments::default(),
        }
    }
}

impl LfoConfig {
    /// Clamp rate, phase shift, and the breakpoint curve.
    pub fn sanitize(&mut self) {
        self.rate = self.rate.map(|rate| sanitize(rate, MIN_RATE, MAX_RATE));
        self.phase_shift = self
            .phase_shift
            .map(|phase_shift| sanitize(phase_shift, MIN_PHASE_SHIFT, MAX_PHASE_SHIFT));
        self.segments.sanitize();
    }
}
