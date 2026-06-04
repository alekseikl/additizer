use std::sync::Arc;

use parking_lot::Mutex;

use crate::synth_engine::{Input, ModuleId, StereoSample, SynthEngine};

use super::{Amplifier, Config};
use super::link::UiEnd;

pub struct UiBridge {
    synth: Arc<Mutex<SynthEngine>>,
    module_id: ModuleId,
    ui_end: Option<UiEnd>,
    config: Config,
}

impl UiBridge {
    pub fn create(module_id: ModuleId, synth: Arc<Mutex<SynthEngine>>) -> Option<Self> {
        let mut synth_lock = synth.lock();
        let amp = synth_lock.get_typed_module_mut::<Amplifier>(module_id)?;
        let ui_end = amp.take_ui_end()?;
        let config = amp.get_config();

        drop(synth_lock);

        Some(Self {
            synth,
            module_id,
            ui_end: Some(ui_end),
            config,
        })
    }

    pub fn module_id(&self) -> ModuleId {
        self.module_id
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn set_param(&mut self, input: Input, value: StereoSample) {
        if self.ui_end.as_mut().unwrap().set_param(input, value) && input == Input::Gain {
            self.config.gain = value;
        }
    }
}

impl Drop for UiBridge {
    fn drop(&mut self) {
        let mut synth_lock = self.synth.lock();

        if let Some(amp) = synth_lock.get_typed_module_mut::<Amplifier>(self.module_id) {
            amp.return_ui_end(self.ui_end.take().unwrap());
        }
    }
}
