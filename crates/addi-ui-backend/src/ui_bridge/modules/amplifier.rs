use crate::ui_bridge::ModuleUiBridge;
use addi_engine::amplifier::{Amplifier, AmplifierConfig, AmplifierLinks, AmplifierUiEnd};
use addi_engine::{Input, StereoSample};

pub struct AmplifierUiBridge<L: AmplifierLinks = crate::links::amplifier::Links> {
    ui_end: L::UiEnd,
    config: AmplifierConfig,
}

impl<L: AmplifierLinks> AmplifierUiBridge<L> {
    pub fn new(ui_end: L::UiEnd, config: AmplifierConfig) -> Self {
        Self { ui_end, config }
    }

    pub fn try_new(amp: &mut Amplifier<L>) -> Option<Self> {
        Some(Self::new(amp.take_ui_end()?, amp.get_config()))
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
                Input::LinLevel => self.config.gain = value,
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
