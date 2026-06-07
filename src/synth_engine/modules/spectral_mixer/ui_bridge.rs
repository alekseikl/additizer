use std::sync::Arc;

use parking_lot::Mutex;

use crate::synth_engine::{Input, MixType, ModuleId, StereoSample, SynthEngine, VolumeType};

use super::link::UiEnd;
use super::{SpectralMixer, SpectralMixerConfig};

pub struct UiBridge {
    synth: Arc<Mutex<SynthEngine>>,
    module_id: ModuleId,
    ui_end: Option<UiEnd>,
    config: SpectralMixerConfig,
}

impl UiBridge {
    pub fn create(module_id: ModuleId, synth: Arc<Mutex<SynthEngine>>) -> Option<Self> {
        let mut synth_lock = synth.lock();
        let mixer = synth_lock.get_typed_module_mut::<SpectralMixer>(module_id)?;
        let ui_end = mixer.take_ui_end()?;
        let config = mixer.get_config();

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

    pub fn config(&self) -> &SpectralMixerConfig {
        &self.config
    }

    pub fn set_param(&mut self, input: Input, value: StereoSample) {
        if !self.ui_end.as_mut().unwrap().set_param(input, value) {
            return;
        }

        match input {
            Input::Gain => self.config.output_gain = value,
            Input::Level => self.config.output_level = value,
            Input::GainMix(idx) => self.config.inputs[idx as usize].gain = value,
            Input::LevelMix(idx) => self.config.inputs[idx as usize].level = value,
            _ => (),
        }
    }

    pub fn set_num_inputs(&mut self, num_inputs: u8) {
        if self.ui_end.as_mut().unwrap().set_num_inputs(num_inputs) {
            self.config.num_inputs = num_inputs;
        }
    }

    pub fn set_mix_type(&mut self, input_idx: u8, mix_type: MixType) {
        if self
            .ui_end
            .as_mut()
            .unwrap()
            .set_mix_type(input_idx, mix_type)
        {
            self.config.inputs[input_idx as usize].mix_type = mix_type;
        }
    }

    pub fn set_volume_type(&mut self, input_idx: u8, volume_type: VolumeType) {
        if self
            .ui_end
            .as_mut()
            .unwrap()
            .set_volume_type(input_idx, volume_type)
        {
            self.config.inputs[input_idx as usize].volume_type = volume_type;
        }
    }

    pub fn set_output_volume_type(&mut self, volume_type: VolumeType) {
        if self
            .ui_end
            .as_mut()
            .unwrap()
            .set_output_volume_type(volume_type)
        {
            self.config.output_volume_type = volume_type;
        }
    }
}

impl Drop for UiBridge {
    fn drop(&mut self) {
        let mut synth_lock = self.synth.lock();

        if let Some(mixer) = synth_lock.get_typed_module_mut::<SpectralMixer>(self.module_id) {
            mixer.return_ui_end(self.ui_end.take().unwrap());
        }
    }
}
