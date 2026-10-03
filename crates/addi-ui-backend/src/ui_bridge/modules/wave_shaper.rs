use crate::ui_bridge::ModuleUiBridge;
use addi_engine::wave_shaper::{
    ShaperType, WaveShaper, WaveShaperConfig, WaveShaperLinks, WaveShaperUiEnd,
};
use addi_engine::{Input, StereoSample};

pub struct WaveShaperUiBridge<L: WaveShaperLinks = crate::links::wave_shaper::Links> {
    ui_end: L::UiEnd,
    config: WaveShaperConfig,
}

impl<L: WaveShaperLinks> WaveShaperUiBridge<L> {
    pub fn try_new(shaper: &mut WaveShaper<L>) -> Option<Self> {
        Some(Self {
            ui_end: shaper.take_ui_end()?,
            config: shaper.get_config(),
        })
    }

    pub fn config(&self) -> &WaveShaperConfig {
        &self.config
    }

    pub fn set_param(&mut self, input: Input, value: StereoSample) {
        if !self.ui_end.set_param(input, value) {
            return;
        }

        match input {
            Input::Distortion => self.config.distortion = value,
            Input::ClippingLevel => self.config.clipping_level = value,
            _ => (),
        }
    }

    pub fn set_shaper_type(&mut self, shaper_type: ShaperType) {
        if self.ui_end.set_shaper_type(shaper_type) {
            self.config.shaper_type = shaper_type;
        }
    }
}

impl<L: WaveShaperLinks> ModuleUiBridge for WaveShaperUiBridge<L> {
    fn update(&mut self) -> bool {
        false
    }
}
