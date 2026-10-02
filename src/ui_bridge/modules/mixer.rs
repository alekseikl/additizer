use crate::synth_engine::mixer::{Mixer, MixerConfig, MixerLinks, MixerUiEnd, UiUpdate};
use crate::synth_engine::{Input, StereoSample, VolumeType, synth_module::ModuleUiBridge};

pub struct MixerUiBridge<L: MixerLinks = crate::links::mixer::Links> {
    ui_end: L::UiEnd,
    config: MixerConfig,
}

impl<L: MixerLinks> MixerUiBridge<L> {
    pub fn try_new(mixer: &mut Mixer<L>) -> Option<Self> {
        Some(Self {
            ui_end: mixer.take_ui_end()?,
            config: mixer.get_config(),
        })
    }

    pub fn config(&self) -> &MixerConfig {
        &self.config
    }

    pub fn get_out_volume(&mut self) -> StereoSample {
        self.ui_end.get_out_volume()
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

impl<L: MixerLinks> ModuleUiBridge for MixerUiBridge<L> {
    fn update(&mut self) -> bool {
        let mut routing_refresh = false;

        while let Some(update) = self.ui_end.pop_update() {
            match update {
                UiUpdate::RefreshRouting => routing_refresh = true,
            }
        }

        routing_refresh
    }
}
