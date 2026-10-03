use addi_ui_backend::ui_bridge::modules::spectral_band_select::SpectralBandSelectUiBridge;
use egui::emath::GuiRounding;

use crate::{frequency_bins::FrequencyBins, grid::WidgetCtx};
use addi_engine::ModuleId;
use addi_ui_backend::ui_bridge::ModuleBridge;

use super::GridWidgetContent;

const PADDING: f32 = 4.0;

#[derive(Default)]
pub struct SpectralBandSelectWidget {
    bins: FrequencyBins,
}

impl SpectralBandSelectWidget {
    fn select_ui(&mut self, ui: &mut egui::Ui, select_bridge: &mut SpectralBandSelectUiBridge) {
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
            .paint(ui.painter(), rect, select_bridge.get_spectrum());
    }
}

impl GridWidgetContent for SpectralBandSelectWidget {
    fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut WidgetCtx, module_id: ModuleId) {
        ctx.bridge
            .with_module_bridge(module_id, |_bridge, module_bridge| {
                if let ModuleBridge::SpectralBandSelect(select_bridge) = module_bridge {
                    self.select_ui(ui, select_bridge);
                }
            });
    }
}
