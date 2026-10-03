use crate::ui_bridge::ModuleUiBridge;
use addi_engine::lfo::{Lfo, LfoConfig, LfoLinks, LfoShape, LfoUiEnd};
use addi_engine::{Input, Sample, StereoSample};

pub struct LfoUiBridge<L: LfoLinks = crate::links::lfo::Links> {
    ui_end: L::UiEnd,
    config: LfoConfig,
}

impl<L: LfoLinks> LfoUiBridge<L> {
    pub fn try_new(lfo: &mut Lfo<L>) -> Option<Self> {
        Some(Self {
            ui_end: lfo.take_ui_end()?,
            config: lfo.get_config(),
        })
    }

    pub fn config(&self) -> &LfoConfig {
        &self.config
    }

    pub fn get_phase(&mut self) -> Sample {
        self.ui_end.get_phase()
    }

    pub fn set_param(&mut self, input: Input, value: StereoSample) {
        if !self.ui_end.set_param(input, value) {
            return;
        }

        match input {
            Input::LowFrequency => self.config.frequency = value,
            Input::PhaseShift => self.config.phase_shift = value,
            Input::Skew => self.config.skew = value,
            _ => (),
        }
    }

    pub fn set_shape(&mut self, shape: LfoShape) {
        if self.ui_end.set_shape(shape) {
            self.config.shape = shape;
        }
    }

    pub fn set_bipolar(&mut self, value: bool) {
        if self.ui_end.set_bipolar(value) {
            self.config.bipolar = value;
        }
    }

    pub fn set_steal_phase(&mut self, value: bool) {
        if self.ui_end.set_steal_phase(value) {
            self.config.steal_phase = value;
        }
    }

    pub fn set_smooth_time(&mut self, value: StereoSample) {
        if self.ui_end.set_smooth_time(value) {
            self.config.smooth_time = value;
        }
    }
}

impl<L: LfoLinks> ModuleUiBridge for LfoUiBridge<L> {
    fn update(&mut self) -> bool {
        false
    }
}
