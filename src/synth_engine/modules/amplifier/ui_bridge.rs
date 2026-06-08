use std::sync::Arc;

use parking_lot::Mutex;

use crate::synth_engine::{
    Input, ModuleId, StereoSample, SynthEngine, synth_module::ModuleUiBridge,
};

use super::link::UiEnd;
use super::{Amplifier, AmplifierConfig};

pub struct AmplifierUiBridge {
    ui_end: Option<UiEnd>,
    config: AmplifierConfig,
}

impl AmplifierUiBridge {
    pub fn create(module_id: ModuleId, synth: Arc<Mutex<SynthEngine>>) -> Option<Self> {
        let mut synth_lock = synth.lock();
        let amp = synth_lock.get_typed_module_mut::<Amplifier>(module_id)?;
        let ui_end = amp.ui_end.take()?;
        let config = amp.get_config();

        drop(synth_lock);

        Some(Self {
            ui_end: Some(ui_end),
            config,
        })
    }

    pub fn config(&self) -> &AmplifierConfig {
        &self.config
    }

    pub fn set_param(&mut self, input: Input, value: StereoSample) {
        if self.ui_end.as_mut().unwrap().set_param(input, value) && input == Input::Gain {
            self.config.gain = value;
        }
    }
}

impl ModuleUiBridge for AmplifierUiBridge {
    fn update(&mut self) {}
}
