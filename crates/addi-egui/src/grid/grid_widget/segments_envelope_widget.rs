use addi_ui_backend::ui_bridge::modules::segments_envelope::SegmentsEnvelopeUiBridge;

use crate::grid::WidgetCtx;
use crate::segments_editor::RelWidgetPlot;
use addi_engine::ModuleId;
use addi_ui_backend::ui_bridge::ModuleBridge;

use super::GridWidgetContent;

const PADDING: f32 = 4.0;

pub struct SegmentsEnvelopeWidget {
    plot: RelWidgetPlot,
}

impl Default for SegmentsEnvelopeWidget {
    fn default() -> Self {
        Self {
            plot: RelWidgetPlot::envelope(),
        }
    }
}

impl SegmentsEnvelopeWidget {
    fn envelope_ui(
        &mut self,
        ui: &mut egui::Ui,
        envelope_bridge: &mut SegmentsEnvelopeUiBridge,
        has_active_voices: bool,
    ) {
        let size = ui.available_size();
        let response = ui.allocate_response(size, egui::Sense::hover());
        let rect = response.rect.shrink2(egui::vec2(0.0, PADDING));

        if !rect.is_positive() || !ui.is_rect_visible(rect) {
            return;
        }

        let playhead = has_active_voices.then(|| {
            let playhead = envelope_bridge.playhead();

            [
                (playhead.phase.left(), playhead.level.left()),
                (playhead.phase.right(), playhead.level.right()),
            ]
        });

        self.plot
            .paint(ui, rect, &envelope_bridge.config().segments, None, playhead);
    }
}

impl GridWidgetContent for SegmentsEnvelopeWidget {
    fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut WidgetCtx, module_id: ModuleId) {
        let has_active_voices = ctx.bridge.has_active_voices();

        ctx.bridge
            .with_module_bridge(module_id, |_bridge, module_bridge| {
                if let ModuleBridge::SegmentsEnvelope(envelope_bridge) = module_bridge {
                    self.envelope_ui(ui, envelope_bridge, has_active_voices);
                }
            });
    }
}
