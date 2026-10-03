use crate::ui_bridge::modules::spectral_blend::SpectralBlendUiBridge;
use egui::emath::GuiRounding;

use crate::{
    editor::{frequency_bins::FrequencyBins, grid::WidgetCtx},
    synth_engine::{ModuleId, ui_bridge::ModuleBridge},
};

use super::GridWidgetContent;

const PADDING: f32 = 4.0;

#[derive(Default)]
pub struct SpectralBlendWidget {
    bins: FrequencyBins,
}

impl SpectralBlendWidget {
    fn blend_ui(&mut self, ui: &mut egui::Ui, blend_bridge: &mut SpectralBlendUiBridge) {
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
            .paint(ui.painter(), rect, blend_bridge.get_spectrum());
    }
}

impl GridWidgetContent for SpectralBlendWidget {
    fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut WidgetCtx, module_id: ModuleId) {
        ctx.bridge
            .with_module_bridge(module_id, |_bridge, module_bridge| {
                if let ModuleBridge::SpectralBlend(blend_bridge) = module_bridge {
                    self.blend_ui(ui, blend_bridge);
                }
            });
    }
}
