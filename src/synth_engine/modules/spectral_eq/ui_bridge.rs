use crate::synth_engine::{Input, Sample, StereoSample, synth_module::ModuleUiBridge};

use super::SpectralEq;
use super::config::{EqFilter, MAX_EQ_FILTERS, SpectralEqConfig};
use super::link::UiEnd;

pub struct SpectralEqUiBridge {
    ui_end: UiEnd,
    config: SpectralEqConfig,
}

impl SpectralEqUiBridge {
    pub fn try_new(eq: &mut SpectralEq) -> Option<Self> {
        Some(Self {
            ui_end: eq.ui_end.take()?,
            config: eq.get_config(),
        })
    }

    pub fn config(&self) -> &SpectralEqConfig {
        &self.config
    }

    pub fn pitch(&mut self) -> Sample {
        self.ui_end.pitch()
    }

    pub fn set_param(&mut self, input: Input, value: StereoSample) {
        if !self.ui_end.set_param(input, value) {
            return;
        }

        match input {
            Input::Cutoff => self.config.cutoff = value,
            Input::Level => self.config.output_level = value,
            _ => (),
        }
    }

    pub fn set_linear_phase(&mut self, value: bool) {
        if self.ui_end.set_linear_phase(value) {
            self.config.linear_phase = value;
        }
    }

    pub fn set_keytrack(&mut self, value: Sample) {
        if self.ui_end.set_keytrack(value) {
            self.config.keytrack = value;
        }
    }

    pub fn set_q_limit_to(&mut self, value: StereoSample) {
        if self.ui_end.set_q_limit_to(value) {
            self.config.q_limit_to = value;
        }
    }

    pub fn set_q_limit_slope(&mut self, value: StereoSample) {
        if self.ui_end.set_q_limit_slope(value) {
            self.config.q_limit_slope = value;
        }
    }

    pub fn set_filter(&mut self, index: u8, filter: EqFilter) {
        let filter = filter.sanitized();

        if self.ui_end.set_filter(index, filter)
            && let Some(slot) = self.config.filters.get_mut(index as usize)
        {
            *slot = filter;
        }
    }

    pub fn add_filter(&mut self, filter: EqFilter) {
        if self.config.filters.len() >= MAX_EQ_FILTERS {
            return;
        }

        let filter = filter.sanitized();

        if self.ui_end.add_filter(filter) {
            self.config.filters.push(filter);
        }
    }

    pub fn remove_filter(&mut self, index: u8) {
        if self.ui_end.remove_filter(index) {
            let index = index as usize;

            if index < self.config.filters.len() {
                self.config.filters.remove(index);
            }
        }
    }

    pub fn move_filter(&mut self, from: u8, to: u8) {
        let from_index = from as usize;
        let to_index = to as usize;
        let len = self.config.filters.len();

        if from_index >= len || to_index >= len || from_index == to_index {
            return;
        }

        if self.ui_end.move_filter(from, to) {
            let filter = self.config.filters.remove(from_index);
            self.config.filters.insert(to_index, filter);
        }
    }
}

impl ModuleUiBridge for SpectralEqUiBridge {
    fn update(&mut self) -> bool {
        false
    }
}
