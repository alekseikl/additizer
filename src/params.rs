use nih_plug::params::persist::PersistentField;
use nih_plug::prelude::*;
use nih_plug_egui::EguiState;
use parking_lot::Mutex;
use std::sync::Arc;

use crate::{default_scheme::build_default_preset, engine_factory::EngineFactory, preset::Preset};

#[derive(Params)]
pub struct AdditizerParams {
    #[persist = "editor-state"]
    pub editor_state: Arc<EguiState>,

    #[persist = "plugin-preset"]
    pub config: PresetWrapper,

    #[id = "volume"]
    pub volume: Arc<FloatParam>,

    #[id = "float-param-1"]
    pub float_param_1: Arc<FloatParam>,

    #[id = "float-param-2"]
    pub float_param_2: Arc<FloatParam>,

    #[id = "float-param-3"]
    pub float_param_3: Arc<FloatParam>,

    #[id = "float-param-4"]
    pub float_param_4: Arc<FloatParam>,
}

impl Default for AdditizerParams {
    fn default() -> Self {
        Self {
            editor_state: EguiState::from_size(900, 600),
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
            float_param_1: Arc::new(FloatParam::new(
                "Float Param 1",
                0.0,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )),
            float_param_2: Arc::new(FloatParam::new(
                "Float Param 2",
                0.0,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )),
            float_param_3: Arc::new(FloatParam::new(
                "Float Param 3",
                0.0,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )),
            float_param_4: Arc::new(FloatParam::new(
                "Float Param 4",
                0.0,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )),
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
        *self.factory.lock() = Some(factory.clone());

        if let Some(cfg) = self.preset_from_host.lock().as_ref() {
            factory.load_preset(cfg);
        } else {
            factory.load_preset(&build_default_preset());
        }
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
