use std::sync::Arc;

use parking_lot::Mutex;

use crate::synth_engine::{Input, ModuleId, StereoSample, SynthEngine};

use super::{Config, ShaperType, WaveShaper};
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
        let shaper = synth_lock.get_typed_module_mut::<WaveShaper>(module_id)?;
        let ui_end = shaper.take_ui_end()?;
        let config = shaper.get_config();

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
        if !self.ui_end.as_mut().unwrap().set_param(input, value) {
            return;
        }

        match input {
            Input::Distortion => self.config.distortion = value,
            Input::ClippingLevel => self.config.clipping_level = value,
            _ => (),
        }
    }

    pub fn set_shaper_type(&mut self, shaper_type: ShaperType) {
        if self.ui_end.as_mut().unwrap().set_shaper_type(shaper_type) {
            self.config.shaper_type = shaper_type;
        }
    }
}

impl Drop for UiBridge {
    fn drop(&mut self) {
        let mut synth_lock = self.synth.lock();

        if let Some(shaper) = synth_lock.get_typed_module_mut::<WaveShaper>(self.module_id) {
            shaper.return_ui_end(self.ui_end.take().unwrap());
        }
    }
}
