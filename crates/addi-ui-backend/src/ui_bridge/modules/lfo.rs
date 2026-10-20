use crate::ui_bridge::ModuleUiBridge;
use addi_dsp::segments::Segments;
use addi_engine::lfo::{Lfo, LfoConfig, LfoLinks, LfoUiEnd};
use addi_engine::{Input, Smp, StereoSample};

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

    pub fn get_phase(&mut self) -> Smp {
        self.ui_end.get_phase()
    }

    pub fn set_param(&mut self, input: Input, value: StereoSample) {
        if !self.ui_end.set_param(input, value) {
            return;
        }

        match input {
            Input::LowFrequency => self.config.frequency = value,
            Input::PhaseShift => self.config.phase_shift = value,
            _ => (),
        }
    }

    pub fn set_segments(&mut self, segments: Segments) {
        if self.ui_end.set_segments(segments.clone()) {
            self.config.segments = segments;
        }
    }

    pub fn set_steal_phase(&mut self, value: bool) {
        if self.ui_end.set_steal_phase(value) {
            self.config.steal_phase = value;
        }
    }
}

impl<L: LfoLinks> ModuleUiBridge for LfoUiBridge<L> {
    fn update(&mut self) -> bool {
        false
    }
}
