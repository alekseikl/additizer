use std::sync::Arc;

use arc_swap::ArcSwap;
use nih_plug::params::FloatParam;
use parking_lot::Mutex;

use crate::{
    preset::Preset,
    synth_engine::{
        EngineConfig, ExternalParamsBlock, Sample, SynthEngine, ui_bridge::ui_config::UiConfig,
    },
};

pub type EngineHandle = Arc<Mutex<SynthEngine>>;
pub type UiConfigHandle = Arc<Mutex<UiConfig>>;

pub struct EngineFactory {
    external_params: Arc<ExternalParamsBlock>,
    output_level_param: Arc<FloatParam>,
    host_sample_rate: Sample,
    engine: ArcSwap<Mutex<SynthEngine>>,
    ui_config: ArcSwap<Mutex<UiConfig>>,
}

impl EngineFactory {
    pub fn new(
        output_level_param: Arc<FloatParam>,
        external_params: Arc<ExternalParamsBlock>,
        host_sample_rate: Sample,
    ) -> Self {
        Self {
            external_params: external_params.clone(),
            output_level_param: output_level_param.clone(),
            host_sample_rate,
            engine: ArcSwap::from_pointee(Mutex::new(
                SynthEngine::try_new(
                    &EngineConfig::default(),
                    output_level_param.clone(),
                    external_params.clone(),
                    host_sample_rate,
                )
                .unwrap(),
            )),
            ui_config: ArcSwap::from_pointee(Mutex::new(UiConfig::default())),
        }
    }

    pub fn get_engine(&self) -> EngineHandle {
        self.engine.load_full()
    }

    pub fn get_ui_config(&self) -> UiConfigHandle {
        self.ui_config.load_full()
    }

    pub fn engine_changed(&self, cached: &EngineHandle) -> bool {
        !Arc::ptr_eq(&*self.engine.load(), cached)
    }

    pub fn get_preset(&self) -> Preset {
        Preset {
            engine: self.engine.load().lock().get_config(),
            ui: self.ui_config.load().lock().clone(),
        }
    }

    pub fn load_preset(&self, preset: &Preset) -> bool {
        let Some(new_engine) = SynthEngine::try_new(
            &preset.engine,
            self.output_level_param.clone(),
            self.external_params.clone(),
            self.host_sample_rate,
        ) else {
            return false;
        };

        self.ui_config
            .store(Arc::new(Mutex::new(preset.ui.clone())));
        self.engine.store(Arc::new(Mutex::new(new_engine)));
        true
    }
}
