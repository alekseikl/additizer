use crate::synth_engine::spectral_noise::{
    NoiseColor, SpectralNoise, SpectralNoiseConfig, SpectralNoiseLinks, SpectralNoiseUiEnd,
    clamp_bandwidth, clamp_cutoff, clamp_rolloff,
};
use crate::synth_engine::{ComplexSample, Input, StereoSample, synth_module::ModuleUiBridge};

pub struct SpectralNoiseUiBridge<L: SpectralNoiseLinks = crate::links::spectral_noise::Links> {
    ui_end: L::UiEnd,
    config: SpectralNoiseConfig,
}

impl<L: SpectralNoiseLinks> SpectralNoiseUiBridge<L> {
    pub fn try_new(noise: &mut SpectralNoise<L>) -> Option<Self> {
        Some(Self {
            ui_end: noise.take_ui_end()?,
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
        let bandwidth = clamp_bandwidth(bandwidth);

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
        let value = clamp_cutoff(value);

        if self.ui_end.set_cutoff(value) {
            self.config.cutoff = value;
        }
    }

    pub fn set_rolloff(&mut self, value: StereoSample) {
        let value = clamp_rolloff(value);

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

impl<L: SpectralNoiseLinks> ModuleUiBridge for SpectralNoiseUiBridge<L> {
    fn update(&mut self) -> bool {
        false
    }
}
