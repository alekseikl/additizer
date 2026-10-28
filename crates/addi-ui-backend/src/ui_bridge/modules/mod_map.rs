use crate::ui_bridge::ModuleUiBridge;
use addi_dsp::segments::Segments;
use addi_engine::mod_map::{ModMap, ModMapConfig, ModMapLinks, ModMapUiEnd};
use addi_engine::{Input, Smp, StereoSample};

pub struct ModMapUiBridge<L: ModMapLinks = crate::links::mod_map::Links> {
    ui_end: L::UiEnd,
    config: ModMapConfig,
}

impl<L: ModMapLinks> ModMapUiBridge<L> {
    pub fn try_new(module: &mut ModMap<L>) -> Option<Self> {
        Some(Self {
            ui_end: module.take_ui_end()?,
            config: module.get_config(),
        })
    }

    pub fn config(&self) -> &ModMapConfig {
        &self.config
    }

    pub fn probe(&mut self) -> StereoSample {
        self.ui_end.probe()
    }

    pub fn segments_mut(&mut self) -> &mut Segments {
        &mut self.config.segments
    }

    pub fn set_param(&mut self, input: Input, value: StereoSample) {
        if !self.ui_end.set_param(input, value) {
            return;
        }

        if matches!(input, Input::Amount) {
            self.config.amount = value;
        }
    }

    /// Publish the curve from [`segments_mut`](Self::segments_mut) to the audio thread.
    pub fn sync_segments(&mut self) {
        let _ = self.ui_end.set_segments(self.config.segments.clone());
    }

    pub fn set_bipolar(&mut self, value: bool) {
        if self.ui_end.set_bipolar(value) {
            self.config.bipolar = value;
        }
    }

    pub fn set_smooth(&mut self, value: Smp) {
        if self.ui_end.set_smooth(value) {
            self.config.smooth = value;
        }
    }
}

impl<L: ModMapLinks> ModuleUiBridge for ModMapUiBridge<L> {
    fn update(&mut self) -> bool {
        false
    }
}
