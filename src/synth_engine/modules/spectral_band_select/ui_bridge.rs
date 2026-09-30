use crate::synth_engine::{Sample, synth_module::ModuleUiBridge, types::ComplexSample};

use super::config::{MAX_BAND_HZ, MAX_HARMONIC, MAX_HARMONIC_END, MIN_BAND_HZ, MIN_HARMONIC};
use super::link::UiEnd;
use super::{BandSelectMode, SpectralBandSelect, SpectralBandSelectConfig};

pub struct SpectralBandSelectUiBridge {
    ui_end: UiEnd,
    config: SpectralBandSelectConfig,
}

impl SpectralBandSelectUiBridge {
    pub fn try_new(module: &mut SpectralBandSelect) -> Option<Self> {
        Some(Self {
            ui_end: module.ui_end.take()?,
            config: module.get_config(),
        })
    }

    pub fn config(&self) -> &SpectralBandSelectConfig {
        &self.config
    }

    pub fn get_spectrum(&mut self) -> &[ComplexSample] {
        self.ui_end.get_spectrum()
    }

    pub fn set_mode(&mut self, mode: BandSelectMode) {
        if self.ui_end.set_mode(mode) {
            self.config.mode = mode;
        }
    }

    pub fn set_harmonic_from(&mut self, value: u16) {
        let value = value.clamp(MIN_HARMONIC, MAX_HARMONIC);

        if self.ui_end.set_harmonic_from(value) {
            self.config.harmonic_from = value;
        }
    }

    pub fn set_harmonic_to(&mut self, value: u16) {
        let value = value.clamp(MIN_HARMONIC, MAX_HARMONIC_END);

        if self.ui_end.set_harmonic_to(value) {
            self.config.harmonic_to = value;
        }
    }

    pub fn set_freq_from(&mut self, value: Sample) {
        let value = value.clamp(MIN_BAND_HZ, MAX_BAND_HZ);

        if self.ui_end.set_freq_from(value) {
            self.config.freq_from = value;
        }
    }

    pub fn set_freq_to(&mut self, value: Sample) {
        let value = value.clamp(MIN_BAND_HZ, MAX_BAND_HZ);

        if self.ui_end.set_freq_to(value) {
            self.config.freq_to = value;
        }
    }
}

impl ModuleUiBridge for SpectralBandSelectUiBridge {
    fn update(&mut self) -> bool {
        false
    }
}
