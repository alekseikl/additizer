use addi_ui_backend::ui_bridge::modules::spectral_mixer::SpectralMixerUiBridge;
use egui::emath::GuiRounding;

use crate::{frequency_bins::FrequencyBins, grid::WidgetCtx};
use addi_engine::ModuleId;
use addi_ui_backend::ui_bridge::ModuleBridge;

use super::GridWidgetContent;

const PADDING: f32 = 4.0;

#[derive(Default)]
pub struct SpectralMixerWidget {
    bins: FrequencyBins,
}

impl SpectralMixerWidget {
    fn mixer_ui(&mut self, ui: &mut egui::Ui, mixer_bridge: &mut SpectralMixerUiBridge) {
        let size = ui.available_size();
        let response = ui.allocate_response(size, egui::Sense::hover());
        let rect = response
            .rect
            .shrink2(egui::vec2(0.0, PADDING))
            .round_to_pixels(ui.pixels_per_point());

        if !rect.is_positive() || !ui.is_rect_visible(rect) {
            return;
        }

        self.bins
            .paint(ui.painter(), rect, mixer_bridge.get_spectrum());
    }
}

impl GridWidgetContent for SpectralMixerWidget {
    fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut WidgetCtx, module_id: ModuleId) {
        ctx.bridge
            .with_module_bridge(module_id, |_bridge, module_bridge| {
                if let ModuleBridge::SpectralMixer(mixer_bridge) = module_bridge {
                    self.mixer_ui(ui, mixer_bridge);
                }
            });
    }
}
