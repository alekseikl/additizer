use nih_plug::prelude::*;
use std::sync::{Arc, Mutex};
use vizia_plug::ViziaState;

use crate::{VOLUME_POLY_MOD_ID, editor};

#[derive(Params)]
pub struct AdditizerParams {
    #[persist = "editor-state"]
    pub editor_state: Arc<ViziaState>,

    #[persist = "harmonics"]
    pub harmonics: Arc<Mutex<Vec<f32>>>,

    #[persist = "subharmonics"]
    pub subharmonics: Arc<Mutex<Vec<f32>>>,

    #[persist = "tail-harmonics"]
    pub tail_harmonics: Arc<AtomicF32>,

    #[id = "volume"]
    pub volume: FloatParam,
}

impl Default for AdditizerParams {
    fn default() -> Self {
        Self {
            editor_state: editor::default_state(),
            harmonics: Arc::new(Mutex::new(vec![1.0; 32])),
            subharmonics: Arc::new(Mutex::new(vec![0.0; 3])),
            tail_harmonics: Arc::new(AtomicF32::new(1.0)),
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
            .with_poly_modulation_id(VOLUME_POLY_MOD_ID)
            .with_smoother(SmoothingStyle::Linear(3.0))
            .with_step_size(0.01)
            .with_unit(" dB"),
        }
    }
}
