use crate::synth_engine::external_param::{ExternalParam, ExternalParamConfig, ExternalParamLinks, ExternalParamUiEnd, NUM_EXT_PARAMS};
use crate::synth_engine::{Sample, synth_module::ModuleUiBridge};


pub struct ExternalParamUiBridge<L: ExternalParamLinks = crate::links::external_param::Links> {
    ui_end: L::UiEnd,
    config: ExternalParamConfig,
}

impl<L: ExternalParamLinks> ExternalParamUiBridge<L> {
    pub fn try_new(param: &mut ExternalParam<L>) -> Option<Self> {
        Some(Self {
            ui_end: param.take_ui_end()?,
            config: param.get_config(),
        })
    }

    pub fn config(&self) -> &ExternalParamConfig {
        &self.config
    }

    pub fn get_value(&mut self) -> Sample {
        self.ui_end.get_value()
    }

    pub fn select_param(&mut self, index: usize) {
        if self.ui_end.select_param(index) {
            self.config.selected_param_index = index.min(NUM_EXT_PARAMS - 1);
        }
    }

    pub fn set_smooth(&mut self, value: Sample) {
        if self.ui_end.set_smooth(value) {
            self.config.smooth = value;
        }
    }

    pub fn set_sample_on_trigger(&mut self, value: bool) {
        if self.ui_end.set_sample_on_trigger(value) {
            self.config.sample_on_trigger = value;
        }
    }

    pub fn set_make_bipolar(&mut self, value: bool) {
        if self.ui_end.set_make_bipolar(value) {
            self.config.make_bipolar = value;
        }
    }

    pub fn set_polyphonic(&mut self, value: bool) {
        if self.ui_end.set_polyphonic(value) {
            self.config.polyphonic = value;
        }
    }
}

impl<L: ExternalParamLinks> ModuleUiBridge for ExternalParamUiBridge<L> {
    fn update(&mut self) -> bool {
        false
    }
}
