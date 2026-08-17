use nice_plug::editor::dpi::LogicalSize;
use nice_plug::params::persist::PersistentField;
use nice_plug::prelude::*;
use nice_plug_egui::EguiState;
use parking_lot::Mutex;
use std::sync::Arc;

use crate::{
    default_scheme::build_default_preset, engine_factory::EngineFactory, preset::Preset,
    synth_engine::external_param::NUM_FLOAT_PARAMS,
};

#[derive(Params)]
pub struct FloatParamSlot {
    #[id = "ctrl"]
    pub param: Arc<FloatParam>,
}

#[derive(Params)]
pub struct AdditizerParams {
    #[persist = "editor-state"]
    pub editor_state: Arc<EguiState>,

    #[persist = "plugin-preset"]
    pub config: PresetWrapper,

    #[id = "volume"]
    pub volume: Arc<FloatParam>,

    #[nested(array)]
    pub float_params: [FloatParamSlot; NUM_FLOAT_PARAMS],
}

impl Default for AdditizerParams {
    fn default() -> Self {
        Self {
            editor_state: EguiState::from_size(LogicalSize::new(900.0, 600.0)),
            config: PresetWrapper::new(),
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
            float_params: std::array::from_fn(|i| {
                let param = FloatParam::new(
                    format!("Ctrl {}", i + 1),
                    0.0,
                    FloatRange::Linear { min: 0.0, max: 1.0 },
                )
                .with_poly_modulation_id(i as u32);

                FloatParamSlot {
                    param: Arc::new(param),
                }
            }),
        }
    }
}

pub(crate) struct PresetWrapper {
    factory: Mutex<Option<Arc<EngineFactory>>>,
    preset_from_host: Mutex<Option<Preset>>,
}

impl PresetWrapper {
    fn new() -> Self {
        Self {
            factory: Mutex::new(None),
            preset_from_host: Mutex::new(None),
        }
    }

    pub fn set_factory(&self, factory: Arc<EngineFactory>) {
        if let Some(cfg) = self.preset_from_host.lock().as_ref() {
            factory.load_preset(cfg);
        } else {
            factory.load_preset(&build_default_preset());
        }

        *self.factory.lock() = Some(factory);
    }
}

impl<'a> PersistentField<'a, Preset> for PresetWrapper {
    fn set(&self, new_value: Preset) {
        *self.preset_from_host.lock() = Some(new_value);
    }

    fn map<F, R>(&self, f: F) -> R
    where
        F: Fn(&Preset) -> R,
    {
        if let Some(factory) = self.factory.lock().as_ref() {
            let preset = factory.get_preset();

            return f(&preset);
        }

        let config_from_host = self.preset_from_host.lock();

        if let Some(config) = config_from_host.as_ref() {
            return f(config);
        }

        f(&Preset::default())
    }
}
