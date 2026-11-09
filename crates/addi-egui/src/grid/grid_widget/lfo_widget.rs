use addi_ui_backend::ui_bridge::modules::lfo::LfoUiBridge;

use crate::grid::WidgetCtx;
use crate::segments_editor::RelWidgetPlot;
use addi_engine::ModuleId;
use addi_ui_backend::ui_bridge::ModuleBridge;

use super::GridWidgetContent;

const PADDING: f32 = 4.0;

pub struct LfoWidget {
    plot: RelWidgetPlot,
}

impl Default for LfoWidget {
    fn default() -> Self {
        Self {
            plot: RelWidgetPlot::lfo(),
        }
    }
}

impl LfoWidget {
    fn lfo_ui(&mut self, ui: &mut egui::Ui, lfo_bridge: &mut LfoUiBridge, has_active_voices: bool) {
        let size = ui.available_size();
        let response = ui.allocate_response(size, egui::Sense::hover());
        let rect = response.rect.shrink2(egui::vec2(0.0, PADDING));

        if !rect.is_positive() || !ui.is_rect_visible(rect) {
            return;
        }

        let playhead = has_active_voices.then(|| {
            let phase = lfo_bridge.get_phase();
            let segments = &lfo_bridge.config().segments;

            [phase.left(), phase.right()].map(|phase| (phase, segments.value_at(phase)))
        });

        self.plot
            .paint(ui, rect, &lfo_bridge.config().segments, None, playhead);
    }
}

impl GridWidgetContent for LfoWidget {
    fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut WidgetCtx, module_id: ModuleId) {
        let has_active_voices = ctx.bridge.has_active_voices();

        ctx.bridge
            .with_module_bridge(module_id, |_bridge, module_bridge| {
                if let ModuleBridge::Lfo(lfo_bridge) = module_bridge {
                    self.lfo_ui(ui, lfo_bridge, has_active_voices);
                }
            });
    }
}
