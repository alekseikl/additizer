use nih_plug::{params::persist::PersistentField, prelude::*};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::{
    editor::EguiState,
    synth_engine::{Config, StereoSample},
};

#[derive(Serialize, Deserialize)]
pub struct HarmonicsState {
    pub harmonics: Vec<StereoSample>,
    pub tail_harmonics: StereoSample,
    pub val1: f32,
}

#[derive(Params)]
pub struct AdditizerParams {
    #[persist = "editor-state"]
    pub editor_state: Arc<EguiState>,

    #[persist = "plugin-config"]
    pub config: Arc<Config>,

    #[id = "volume"]
    pub volume: Arc<FloatParam>,
}

impl Default for AdditizerParams {
    fn default() -> Self {
        Self {
            editor_state: EguiState::from_size(800, 600),
            config: Default::default(),
            volume: Arc::new(
                FloatParam::new(
                    "Volume",
                    0.0,
                    FloatRange::SymmetricalSkewed {
                        min: util::MINUS_INFINITY_DB,
                        max: 6.0,
                        factor: FloatRange::skew_factor(-1.0),
                        center: 0.0,
                    },
                )
                .with_smoother(SmoothingStyle::Linear(3.0))
                .with_step_size(0.01)
                .with_unit(" dB"),
            ),
        }
    }
}

impl<'a> PersistentField<'a, Config> for Arc<Config> {
    fn set(&self, new_value: Config) {
        *self.routing.lock() = new_value.routing.lock().clone();
        *self.modules.lock() = new_value.modules.lock().clone();
    }

    fn map<F, R>(&self, f: F) -> R
    where
        F: Fn(&Config) -> R,
    {
        f(self)
    }
}
