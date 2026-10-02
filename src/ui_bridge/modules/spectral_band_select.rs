use crate::synth_engine::spectral_band_select::{
    BandSelectMode, MAX_BAND_HZ, MAX_HARMONIC, MAX_HARMONIC_END, MIN_BAND_HZ, MIN_HARMONIC,
    SpectralBandSelect, SpectralBandSelectConfig, SpectralBandSelectLinks, SpectralBandSelectUiEnd,
};
use crate::synth_engine::{Sample, synth_module::ModuleUiBridge, types::ComplexSample};

pub struct SpectralBandSelectUiBridge<
    L: SpectralBandSelectLinks = crate::links::spectral_band_select::Links,
> {
    ui_end: L::UiEnd,
    config: SpectralBandSelectConfig,
}

impl<L: SpectralBandSelectLinks> SpectralBandSelectUiBridge<L> {
    pub fn try_new(module: &mut SpectralBandSelect<L>) -> Option<Self> {
        Some(Self {
            ui_end: module.take_ui_end()?,
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

impl<L: SpectralBandSelectLinks> ModuleUiBridge for SpectralBandSelectUiBridge<L> {
    fn update(&mut self) -> bool {
        false
    }
}
