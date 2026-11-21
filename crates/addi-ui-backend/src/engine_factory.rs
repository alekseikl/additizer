use std::sync::atomic::Ordering;

use atomic_float::AtomicF32;
use parking_lot::Mutex;

use addi_engine::{EngineConfig, Smp, SynthEngine};

use crate::{
    links::PluginLinks,
    preset::{Preset, PresetInfo},
    ui_bridge::{UiBridge, ui_config::UiConfig},
};

pub struct EngineFactory {
    host_sample_rate: AtomicF32,
    /// Preset last published, or last reported by the open editor.
    ///
    /// The live UI config and module state sit on the `UiBridge` the editor holds.
    preset: Mutex<Preset>,
    /// Newly published engine. The audio thread takes it when the mutex is free.
    engine: Mutex<Option<Box<SynthEngine<PluginLinks>>>>,
    /// Built with the latest engine. The UI takes it on the next frame.
    ui_bridge: Mutex<Option<UiBridge>>,
}

impl EngineFactory {
    pub fn new() -> Self {
        Self {
            host_sample_rate: AtomicF32::new(44100.0),
            preset: Mutex::new(Preset::default()),
            engine: Mutex::new(None),
            ui_bridge: Mutex::new(None),
        }
    }

    /// Bridge built by the latest preset or oversampling swap, if the UI has not taken it yet.
    pub fn take_ui_bridge(&self) -> Option<UiBridge> {
        self.ui_bridge.lock().take()
    }

    /// A replacement bridge is waiting for the editor to take it.
    pub fn has_pending_ui_bridge(&self) -> bool {
        self.ui_bridge.lock().is_some()
    }

    /// Takes the published engine when the slot is free, leaving `None`.
    ///
    /// Returns `None` when the slot is empty or `publish` currently holds it.
    pub fn try_take_engine(&self) -> Option<Box<SynthEngine<PluginLinks>>> {
        self.engine.try_lock()?.take()
    }

    pub fn set_host_sample_rate(&self, sample_rate: Smp) {
        self.host_sample_rate.store(sample_rate, Ordering::Release);
    }

    /// Remember the editor's current preset while it holds the bridge.
    pub fn note_preset(&self, preset: Preset) {
        *self.preset.lock() = preset;
    }

    pub fn get_preset(&self) -> Preset {
        self.preset.lock().clone()
    }

    pub fn load_preset(&self, preset: &Preset) -> bool {
        self.publish(&preset.engine, preset.oversampling, preset.ui.clone())
    }

    pub(crate) fn publish(
        &self,
        config: &EngineConfig,
        oversampling: bool,
        ui_config: UiConfig,
    ) -> bool {
        let Some(mut engine) = self.build_engine(config, oversampling) else {
            return false;
        };
        let Some(bridge) = UiBridge::create(&mut engine, ui_config.clone(), oversampling) else {
            return false;
        };

        *self.preset.lock() = Preset {
            info: PresetInfo::default(),
            engine: config.clone(),
            oversampling,
            ui: ui_config,
        };
        *self.engine.lock() = Some(Box::new(engine));
        self.ui_bridge.lock().replace(bridge);
        true
    }

    fn build_engine(
        &self,
        config: &EngineConfig,
        oversampling: bool,
    ) -> Option<SynthEngine<PluginLinks>> {
        SynthEngine::<PluginLinks>::try_new(
            config,
            self.host_sample_rate.load(Ordering::Acquire),
            oversampling,
        )
    }
}
