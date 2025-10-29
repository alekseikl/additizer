use nih_plug::{params::persist::PersistentField, prelude::*};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::{
    editor::egui_integration::EguiState,
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

    #[persist = "harmonics-state"]
    pub harmonics_state: Arc<Mutex<HarmonicsState>>,

    #[id = "volume"]
    pub volume: FloatParam,

    #[id = "unison"]
    pub unison: IntParam,

    #[id = "detune"]
    pub detune: Arc<FloatParam>,

    #[id = "cutoff"]
    pub cutoff: FloatParam,
}

impl Default for AdditizerParams {
    fn default() -> Self {
        Self {
            editor_state: EguiState::from_size(900, 500),
            harmonics_state: Arc::new(Mutex::new(HarmonicsState {
                harmonics: vec![StereoSample::mono(1.0); 40],
                tail_harmonics: StereoSample::mono(1.0),
                val1: 1.0,
            })),
            config: Default::default(),
            volume: FloatParam::new(
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
            unison: IntParam::new("Unison", 3, IntRange::Linear { min: 1, max: 16 }),
            detune: Arc::new(
                FloatParam::new(
                    "Detune",
                    20.0,
                    FloatRange::Linear {
                        min: 0.0,
                        max: 100.0,
                    },
                )
                .with_step_size(0.01),
            ),
            cutoff: FloatParam::new(
                "Cutoff harmonic",
                1.0,
                FloatRange::Skewed {
                    min: 0.0,
                    max: 1023.0,
                    factor: 0.2,
                },
            )
            .with_step_size(0.01),
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
