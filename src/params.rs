use nih_plug::prelude::*;
use parking_lot::Mutex;
use std::sync::Arc;
use vizia_plug::ViziaState;

use crate::editor;

#[derive(Params)]
pub struct AdditizerParams {
    #[persist = "editor-state"]
    pub editor_state: Arc<ViziaState>,

    #[persist = "harmonics"]
    pub harmonics: Arc<Mutex<Vec<f32>>>,

    #[persist = "tail-harmonics"]
    pub tail_harmonics: Arc<AtomicF32>,

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
            editor_state: editor::default_state(),
            harmonics: Arc::new(Mutex::new(vec![1.0; 40])),
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
                1023.0,
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
