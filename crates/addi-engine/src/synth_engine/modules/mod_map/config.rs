use serde::{Deserialize, Serialize};

use addi_dsp::{from_ms, segments::Segments, utils::sanitize};

use crate::synth_engine::{ModuleId, Smp, StereoSample};

/// Smooth time is stored in seconds. `0.05` is 50 ms.
pub const MAX_SMOOTH: Smp = from_ms(50.0);

fn default_amount() -> StereoSample {
    1.0.into()
}

fn identity_segments() -> Segments {
    Segments::from_points(&[(0.0, 0.0), (1.0, 1.0)], false)
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ModMapConfig {
    pub id: ModuleId,
    #[serde(default = "default_amount")]
    pub amount: StereoSample,
    #[serde(default)]
    pub bipolar: bool,
    #[serde(default)]
    pub smooth: Smp,
    #[serde(default = "identity_segments")]
    pub segments: Segments,
}

impl Default for ModMapConfig {
    fn default() -> Self {
        Self {
            id: -1,
            amount: default_amount(),
            bipolar: false,
            smooth: 0.0,
            segments: identity_segments(),
        }
    }
}

impl ModMapConfig {
    pub fn sanitize(&mut self) {
        self.amount = self.amount.map(|amount| sanitize(amount, 0.0, 1.0));
        self.smooth = sanitize(self.smooth, 0.0, MAX_SMOOTH);
        self.segments.sanitize();
    }
}
