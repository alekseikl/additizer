use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use arc_swap::{ArcSwap, ArcSwapOption};
use atomic_float::AtomicF32;
use parking_lot::Mutex;

use addi_engine::{EngineConfig, Smp, SynthEngine};

use crate::{
    links::PluginLinks,
    preset::{Preset, PresetInfo},
    ui_bridge::{UiBridge, ui_config::UiConfig},
};

pub type EngineHandle<E = PluginLinks> = Arc<Mutex<SynthEngine<E>>>;
pub type UiConfigHandle = Arc<Mutex<UiConfig>>;

pub struct EngineFactory {
    host_sample_rate: AtomicF32,
    oversampling: AtomicBool,
    engine: ArcSwapOption<Mutex<SynthEngine<PluginLinks>>>,
    ui_config: ArcSwap<Mutex<UiConfig>>,
    /// Built with the latest engine. The UI takes it on the next frame.
    ui_bridge: Mutex<Option<UiBridge>>,
}

impl EngineFactory {
    pub fn new() -> Self {
        Self {
            host_sample_rate: AtomicF32::new(44100.0),
            oversampling: AtomicBool::new(true),
            engine: ArcSwapOption::empty(),
            ui_config: ArcSwap::from_pointee(Mutex::new(UiConfig::default())),
            ui_bridge: Mutex::new(None),
        }
    }

    /// Bridge built by the latest preset or oversampling swap, if the UI has not taken it yet.
    pub fn take_ui_bridge(&self) -> Option<UiBridge> {
        self.ui_bridge.lock().take()
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

    pub fn set_host_sample_rate(&self, sample_rate: Smp) {
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

        if !self.publish(new_engine, self.get_ui_config()) {
            self.oversampling.store(previous, Ordering::Release);
        }
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

        let ui_config = Arc::new(Mutex::new(preset.ui.clone()));

        if !self.publish(new_engine, ui_config) {
            self.oversampling.store(previous, Ordering::Release);
            return false;
        }

        true
    }

    fn publish(&self, mut engine: SynthEngine<PluginLinks>, ui_config: UiConfigHandle) -> bool {
        let Some(bridge) = UiBridge::create(&mut engine, Arc::clone(&ui_config)) else {
            return false;
        };

        self.ui_config.store(ui_config);
        self.engine.store(Some(Arc::new(Mutex::new(engine))));
        self.ui_bridge.lock().replace(bridge);
        true
    }
}
