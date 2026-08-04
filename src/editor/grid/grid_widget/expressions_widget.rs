use egui::{Rect, Vec2, emath::GuiRounding};

use crate::{
    editor::{control_meter::ControlMeter, fit_label::FitLabel, grid::WidgetCtx},
    synth_engine::{
        ModuleId,
        expressions::ExpressionsUiBridge,
        ui_bridge::{GridVec, ModuleBridge},
    },
};

use super::GridWidgetContent;

const VERT_PADDING: Vec2 = egui::vec2(4.0, 6.0);

#[derive(Default)]
pub struct ExpressionsWidget {
    control_meter: ControlMeter,
}

impl ExpressionsWidget {
    fn expressions_ui(
        &mut self,
        ui: &mut egui::Ui,
        label: String,
        expr_bridge: &mut ExpressionsUiBridge,
    ) {
        ui.add_space(2.0);
        ui.add(FitLabel::new(&label, "Exp"));

        let size = ui.available_size();
        let response = ui.allocate_response(size, egui::Sense::hover());
        let rect = Rect::from_min_max(
            response.rect.left_top() + egui::vec2(0.0, VERT_PADDING.x),
            response.rect.right_bottom() - egui::vec2(0.0, VERT_PADDING.y),
        )
        .round_to_pixels(ui.pixels_per_point());

        if !rect.is_positive() || !ui.is_rect_visible(rect) {
            return;
        }

        self.control_meter.paint_mono(
            &ui.painter().with_clip_rect(rect),
            rect,
            expr_bridge.get_value(),
        );
    }
}

impl GridWidgetContent for ExpressionsWidget {
    fn grid_size(&self) -> GridVec {
        GridVec::new(2, 2)
    }

    fn show_label(&self) -> bool {
        false
    }

    fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut WidgetCtx, module_id: ModuleId) {
        let label = ctx.bridge.display_module_label(module_id);

        ctx.bridge
            .with_module_bridge(module_id, |_bridge, module_bridge| {
                if let ModuleBridge::Expressions(expr_bridge) = module_bridge {
                    self.expressions_ui(ui, label, expr_bridge);
                }
            });
    }
}
