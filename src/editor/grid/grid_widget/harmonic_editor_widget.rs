use crate::{
    editor::{gain_bars, grid::WidgetCtx},
    synth_engine::{ModuleId, harmonic_editor::HarmonicEditorUiBridge, ui_bridge::ModuleBridge},
};

use super::GridWidgetContent;

pub struct HarmonicEditorWidget {}

impl HarmonicEditorWidget {
    fn editor_ui(&mut self, ui: &mut egui::Ui, editor_bridge: &mut HarmonicEditorUiBridge) {
        gain_bars::paint_gain_bars(ui, editor_bridge.get_spectrum());
    }
}

impl GridWidgetContent for HarmonicEditorWidget {
    fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut WidgetCtx, module_id: ModuleId) {
        ctx.bridge
            .with_module_bridge(module_id, |_bridge, module_bridge| {
                if let ModuleBridge::HarmonicEditor(editor_bridge) = module_bridge {
                    self.editor_ui(ui, editor_bridge);
                }
            });
    }
}
