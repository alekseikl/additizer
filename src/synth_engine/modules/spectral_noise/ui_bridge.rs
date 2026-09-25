use crate::synth_engine::{ComplexSample, Input, StereoSample, synth_module::ModuleUiBridge};

use super::SpectralNoise;
use super::config::{NoiseColor, SpectralNoiseConfig};
use super::link::UiEnd;

pub struct SpectralNoiseUiBridge {
    ui_end: UiEnd,
    config: SpectralNoiseConfig,
}

impl SpectralNoiseUiBridge {
    pub fn try_new(noise: &mut SpectralNoise) -> Option<Self> {
        Some(Self {
            ui_end: noise.ui_end.take()?,
            config: noise.get_config(),
        })
    }

    pub fn config(&self) -> &SpectralNoiseConfig {
        &self.config
    }

    pub fn get_display_spectrum(&mut self) -> &[ComplexSample] {
        self.ui_end.get_display_spectrum()
    }

    pub fn set_color(&mut self, color: NoiseColor) {
        if self.ui_end.set_color(color) {
            self.config.color = color;
        }
    }

    pub fn set_bandwidth(&mut self, bandwidth: i32) {
        let bandwidth = SpectralNoise::clamp_bandwidth(bandwidth);

        if self.ui_end.set_bandwidth(bandwidth) {
            self.config.bandwidth = bandwidth;
        }
    }

    pub fn set_param(&mut self, input: Input, value: StereoSample) {
        if !self.ui_end.set_param(input, value) {
            return;
        }

        match input {
            Input::Amount => self.config.amount = value,
            Input::Level => self.config.level = value,
            _ => (),
        }
    }

    pub fn set_stereo(&mut self, stereo: bool) {
        if self.ui_end.set_stereo(stereo) {
            self.config.stereo = stereo;
        }
    }

    pub fn set_cutoff(&mut self, value: StereoSample) {
        let value = SpectralNoise::clamp_cutoff(value);

        if self.ui_end.set_cutoff(value) {
            self.config.cutoff = value;
        }
    }

    pub fn set_rolloff(&mut self, value: StereoSample) {
        let value = SpectralNoise::clamp_rolloff(value);

        if self.ui_end.set_rolloff(value) {
            self.config.rolloff = value;
        }
    }

    pub fn set_steal_phase(&mut self, steal_phase: bool) {
        if self.ui_end.set_steal_phase(steal_phase) {
            self.config.steal_phase = steal_phase;
        }
    }
}

impl ModuleUiBridge for SpectralNoiseUiBridge {
    fn update(&mut self) -> bool {
        false
    }
}
