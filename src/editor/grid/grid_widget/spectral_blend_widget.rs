use crate::{
    editor::{gain_bars, grid::WidgetCtx},
    synth_engine::{ModuleId, spectral_blend::SpectralBlendUiBridge, ui_bridge::ModuleBridge},
};

use super::GridWidgetContent;

pub struct SpectralBlendWidget {}

impl SpectralBlendWidget {
    fn blend_ui(&mut self, ui: &mut egui::Ui, blend_bridge: &mut SpectralBlendUiBridge) {
        gain_bars::paint_gain_bars(ui, blend_bridge.get_spectrum());
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
