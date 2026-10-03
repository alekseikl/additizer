use crate::ui_bridge::modules::external_param::ExternalParamUiBridge;
use egui::{Rect, Vec2, emath::GuiRounding};

use crate::{
    editor::{control_meter::ControlMeter, fit_label::FitLabel, grid::WidgetCtx},
    ui_bridge::{GridVec, ModuleBridge},
};
use addi_engine::ModuleId;

use super::GridWidgetContent;

const VERT_PADDING: Vec2 = egui::vec2(4.0, 6.0);

#[derive(Default)]
pub struct ExternalParamWidget {
    control_meter: ControlMeter,
}

impl ExternalParamWidget {
    fn external_param_ui(
        &mut self,
        ui: &mut egui::Ui,
        label: String,
        param_bridge: &mut ExternalParamUiBridge,
    ) {
        ui.add_space(2.0);
        ui.add(FitLabel::new(&label, "Ext"));

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
            param_bridge.get_value().clamp(0.0, 1.0),
        );
    }
}

impl GridWidgetContent for ExternalParamWidget {
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
                if let ModuleBridge::ExternalParam(param_bridge) = module_bridge {
                    self.external_param_ui(ui, label, param_bridge);
                }
            });
    }
}
