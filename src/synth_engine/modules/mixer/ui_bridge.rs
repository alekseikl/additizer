use crate::synth_engine::{Input, StereoSample, VolumeType, synth_module::ModuleUiBridge};

use super::link::UiEnd;
use super::{Mixer, MixerConfig};

pub struct MixerUiBridge {
    ui_end: UiEnd,
    config: MixerConfig,
}

impl MixerUiBridge {
    pub fn try_new(mixer: &mut Mixer) -> Option<Self> {
        Some(Self {
            ui_end: mixer.ui_end.take()?,
            config: mixer.get_config(),
        })
    }

    pub fn config(&self) -> &MixerConfig {
        &self.config
    }

    pub fn set_param(&mut self, input: Input, value: StereoSample) {
        if !self.ui_end.set_param(input, value) {
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
        if self.ui_end.set_num_inputs(num_inputs) {
            self.config.num_inputs = num_inputs;
        }
    }

    pub fn set_volume_type(&mut self, input_idx: u8, volume_type: VolumeType) {
        if self.ui_end.set_volume_type(input_idx, volume_type) {
            self.config.inputs[input_idx as usize].volume_type = volume_type;
        }
    }

    pub fn set_output_volume_type(&mut self, volume_type: VolumeType) {
        if self.ui_end.set_output_volume_type(volume_type) {
            self.config.output_volume_type = volume_type;
        }
    }
}

impl ModuleUiBridge for MixerUiBridge {
    fn update(&mut self) {}
}
