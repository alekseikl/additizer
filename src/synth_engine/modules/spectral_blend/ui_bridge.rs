use std::sync::Arc;

use parking_lot::Mutex;

use crate::synth_engine::{
    Input, ModuleId, StereoSample, SynthEngine, synth_module::ModuleUiBridge,
};

use super::link::UiEnd;
use super::{SpectralBlend, SpectralBlendConfig};

pub struct SpectralBlendUiBridge {
    ui_end: Option<UiEnd>,
    config: SpectralBlendConfig,
}

impl SpectralBlendUiBridge {
    pub fn create(module_id: ModuleId, synth: Arc<Mutex<SynthEngine>>) -> Option<Self> {
        let mut synth_lock = synth.lock();
        let blend = synth_lock.get_typed_module_mut::<SpectralBlend>(module_id)?;
        let ui_end = blend.ui_end.take()?;
        let config = blend.get_config();

        drop(synth_lock);

        Some(Self {
            ui_end: Some(ui_end),
            config,
        })
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

impl ModuleUiBridge for SpectralBlendUiBridge {
    fn update(&mut self) {}
}
