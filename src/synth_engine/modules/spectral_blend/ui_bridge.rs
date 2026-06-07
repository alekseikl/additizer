use std::sync::Arc;

use parking_lot::Mutex;

use crate::synth_engine::{Input, ModuleId, StereoSample, SynthEngine};

use super::link::UiEnd;
use super::{SpectralBlend, SpectralBlendConfig};

pub struct UiBridge {
    synth: Arc<Mutex<SynthEngine>>,
    module_id: ModuleId,
    ui_end: Option<UiEnd>,
    config: SpectralBlendConfig,
}

impl UiBridge {
    pub fn create(module_id: ModuleId, synth: Arc<Mutex<SynthEngine>>) -> Option<Self> {
        let mut synth_lock = synth.lock();
        let blend = synth_lock.get_typed_module_mut::<SpectralBlend>(module_id)?;
        let ui_end = blend.take_ui_end()?;
        let config = blend.get_config();

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

    pub fn config(&self) -> &SpectralBlendConfig {
        &self.config
    }

    pub fn set_param(&mut self, input: Input, value: StereoSample) {
        if self.ui_end.as_mut().unwrap().set_param(input, value) && input == Input::Blend {
            self.config.blend = value;
        }
    }
}

impl Drop for UiBridge {
    fn drop(&mut self) {
        let mut synth_lock = self.synth.lock();

        if let Some(blend) = synth_lock.get_typed_module_mut::<SpectralBlend>(self.module_id) {
            blend.return_ui_end(self.ui_end.take().unwrap());
        }
    }
}
