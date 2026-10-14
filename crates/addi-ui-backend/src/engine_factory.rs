use std::sync::{Arc, atomic::Ordering};

use arc_swap::{ArcSwap, ArcSwapOption};
use atomic_float::AtomicF32;
use parking_lot::Mutex;

use addi_engine::{Sample, SynthEngine};

use crate::{
    links::PluginLinks,
    preset::{Preset, PresetInfo},
    ui_bridge::ui_config::UiConfig,
};

pub type EngineHandle<E = PluginLinks> = Arc<Mutex<SynthEngine<E>>>;
pub type UiConfigHandle = Arc<Mutex<UiConfig>>;

pub struct EngineFactory {
    host_sample_rate: AtomicF32,
    engine: ArcSwapOption<Mutex<SynthEngine<PluginLinks>>>,
    ui_config: ArcSwap<Mutex<UiConfig>>,
}

impl EngineFactory {
    pub fn new() -> Self {
        Self {
            host_sample_rate: AtomicF32::new(44100.0),
            engine: ArcSwapOption::empty(),
            ui_config: ArcSwap::from_pointee(Mutex::new(UiConfig::default())),
        }
    }

    pub fn get_engine(&self) -> Option<EngineHandle> {
        self.engine.load_full()
    }

    pub fn get_ui_config(&self) -> UiConfigHandle {
        self.ui_config.load_full()
    }

    pub fn engine_changed(&self, cached: &EngineHandle) -> bool {
        self.engine
            .load()
            .as_ref()
            .is_none_or(|current| !Arc::ptr_eq(current, cached))
    }

    pub fn set_host_sample_rate(&self, sample_rate: Sample) {
        self.host_sample_rate.store(sample_rate, Ordering::Release);
    }

    pub fn get_preset(&self) -> Preset {
        let Some(engine) = self.engine.load_full() else {
            return Preset::default();
        };

        Preset {
            info: PresetInfo::default(),
            engine: engine.lock().get_config(),
            ui: self.ui_config.load().lock().clone(),
        }
    }

    pub fn load_preset(&self, preset: &Preset) -> bool {
        let Some(new_engine) = SynthEngine::<PluginLinks>::try_new(
            &preset.engine,
            self.host_sample_rate.load(Ordering::Acquire),
        ) else {
            return false;
        };

        self.ui_config
            .store(Arc::new(Mutex::new(preset.ui.clone())));
        self.engine.store(Some(Arc::new(Mutex::new(new_engine))));
        true
    }
}
