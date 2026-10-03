use crate::ui_bridge::ModuleUiBridge;
use addi_engine::amplifier::{Amplifier, AmplifierConfig, AmplifierLinks, AmplifierUiEnd};
use addi_engine::{Input, StereoSample};

pub struct AmplifierUiBridge<L: AmplifierLinks = crate::links::amplifier::Links> {
    ui_end: L::UiEnd,
    config: AmplifierConfig,
}

impl<L: AmplifierLinks> AmplifierUiBridge<L> {
    pub fn try_new(amp: &mut Amplifier<L>) -> Option<Self> {
        Some(Self {
            ui_end: amp.take_ui_end()?,
            config: amp.get_config(),
        })
    }

    pub fn config(&self) -> &AmplifierConfig {
        &self.config
    }

    pub fn get_out_volume(&mut self) -> StereoSample {
        self.ui_end.get_out_volume()
    }

    pub fn set_param(&mut self, input: Input, value: StereoSample) {
        if self.ui_end.set_param(input, value) {
            match input {
                Input::Gain => self.config.gain = value,
                Input::Level => self.config.level = value,
                Input::Pan => self.config.pan = value,
                _ => (),
            }
        }
    }
}

impl<L: AmplifierLinks> ModuleUiBridge for AmplifierUiBridge<L> {
    fn update(&mut self) -> bool {
        false
    }
}
