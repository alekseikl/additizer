use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use arc_swap::{ArcSwap, ArcSwapOption};
use atomic_float::AtomicF32;
use parking_lot::Mutex;

use addi_engine::{EngineConfig, Sample, SynthEngine};

use crate::{
    links::PluginLinks,
    preset::{Preset, PresetInfo},
    ui_bridge::ui_config::UiConfig,
};

pub type EngineHandle<E = PluginLinks> = Arc<Mutex<SynthEngine<E>>>;
pub type UiConfigHandle = Arc<Mutex<UiConfig>>;

pub struct EngineFactory {
    host_sample_rate: AtomicF32,
    oversampling: AtomicBool,
    engine: ArcSwapOption<Mutex<SynthEngine<PluginLinks>>>,
    ui_config: ArcSwap<Mutex<UiConfig>>,
}

impl EngineFactory {
    pub fn new() -> Self {
        Self {
            host_sample_rate: AtomicF32::new(44100.0),
            oversampling: AtomicBool::new(true),
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

    pub fn oversampling(&self) -> bool {
        self.oversampling.load(Ordering::Acquire)
    }

    pub fn set_oversampling(&self, oversampling: bool) {
        let previous = self.oversampling.swap(oversampling, Ordering::AcqRel);
        if previous == oversampling {
            return;
        }

        let Some(engine) = self.get_engine() else {
            return;
        };
        let config = engine.lock().get_config();

        let Some(new_engine) = self.build_engine(&config) else {
            self.oversampling.store(previous, Ordering::Release);
            return;
        };

        self.engine.store(Some(Arc::new(Mutex::new(new_engine))));
    }

    fn build_engine(&self, config: &EngineConfig) -> Option<SynthEngine<PluginLinks>> {
        SynthEngine::<PluginLinks>::try_new(
            config,
            self.host_sample_rate.load(Ordering::Acquire),
            self.oversampling.load(Ordering::Acquire),
        )
    }

    pub fn get_preset(&self) -> Preset {
        let Some(engine) = self.engine.load_full() else {
            return Preset::default();
        };

        Preset {
            info: PresetInfo::default(),
            engine: engine.lock().get_config(),
            oversampling: self.oversampling(),
            ui: self.ui_config.load().lock().clone(),
        }
    }

    pub fn load_preset(&self, preset: &Preset) -> bool {
        let previous = self
            .oversampling
            .swap(preset.oversampling, Ordering::AcqRel);

        let Some(new_engine) = self.build_engine(&preset.engine) else {
            self.oversampling.store(previous, Ordering::Release);
            return false;
        };

        self.ui_config
            .store(Arc::new(Mutex::new(preset.ui.clone())));
        self.engine.store(Some(Arc::new(Mutex::new(new_engine))));
        true
    }
}
