use crate::ui_bridge::ModuleUiBridge;
use addi_engine::spectral_blend::{
    SpectralBlend, SpectralBlendConfig, SpectralBlendLinks, SpectralBlendUiEnd,
};
use addi_engine::{Input, StereoSample, types::ComplexSample};

pub struct SpectralBlendUiBridge<L: SpectralBlendLinks = crate::links::spectral_blend::Links> {
    ui_end: L::UiEnd,
    config: SpectralBlendConfig,
}

impl<L: SpectralBlendLinks> SpectralBlendUiBridge<L> {
    pub fn try_new(blend: &mut SpectralBlend<L>) -> Option<Self> {
        Some(Self {
            ui_end: blend.take_ui_end()?,
            config: blend.get_config(),
        })
    }

    pub fn config(&self) -> &SpectralBlendConfig {
        &self.config
    }

    pub fn get_spectrum(&mut self) -> &[ComplexSample] {
        self.ui_end.get_spectrum()
    }

    pub fn set_param(&mut self, input: Input, value: StereoSample) {
        if self.ui_end.set_param(input, value) && input == Input::Blend {
            self.config.blend = value;
        }
    }
}

impl<L: SpectralBlendLinks> ModuleUiBridge for SpectralBlendUiBridge<L> {
    fn update(&mut self) -> bool {
        false
    }
}
