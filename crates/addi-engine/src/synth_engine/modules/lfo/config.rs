use serde::{Deserialize, Serialize};

use addi_dsp::{segments::Segments, utils::sanitize};

use crate::synth_engine::{ModuleId, Smp, StereoSample};

pub(super) const MIN_FREQUENCY: Smp = -100.0;
pub(super) const MAX_FREQUENCY: Smp = 100.0;
pub(super) const MIN_PHASE_SHIFT: Smp = -1.0;
pub(super) const MAX_PHASE_SHIFT: Smp = 1.0;

fn default_frequency() -> StereoSample {
    1.0.into()
}

#[derive(Clone, Serialize, Deserialize)]
pub struct LfoConfig {
    pub id: ModuleId,
    #[serde(default)]
    pub steal_phase: bool,
    #[serde(default = "default_frequency")]
    pub frequency: StereoSample,
    #[serde(default)]
    pub phase_shift: StereoSample,
    #[serde(default)]
    pub segments: Segments,
}

impl Default for LfoConfig {
    fn default() -> Self {
        Self {
            id: -1,
            steal_phase: false,
            frequency: 1.0.into(),
            phase_shift: 0.0.into(),
            segments: Segments::default(),
        }
    }
}

impl LfoConfig {
    /// Clamp frequency, phase shift, and the breakpoint curve.
    pub fn sanitize(&mut self) {
        self.frequency = self
            .frequency
            .map(|frequency| sanitize(frequency, MIN_FREQUENCY, MAX_FREQUENCY));
        self.phase_shift = self
            .phase_shift
            .map(|phase_shift| sanitize(phase_shift, MIN_PHASE_SHIFT, MAX_PHASE_SHIFT));
        self.segments.sanitize();
    }
}
