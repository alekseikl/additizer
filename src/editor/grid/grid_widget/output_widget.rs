use egui::{Rect, Vec2, emath::GuiRounding};

use crate::{
    editor::{fit_label::FitLabel, grid::WidgetCtx, volume_meter::VolumeMeter},
    synth_engine::{
        ModuleId, ModuleType,
        ui_bridge::{GridVec, UiBridge},
    },
};

use super::GridWidgetContent;

const VERT_PADDING: Vec2 = egui::vec2(4.0, 6.0);

#[derive(Default)]
pub struct OutputWidget {
    volume_meter: VolumeMeter,
}

impl OutputWidget {
    fn output_ui(&mut self, ui: &mut egui::Ui, bridge: &mut UiBridge) {
        ui.add_space(2.0);
        ui.add(FitLabel::new(ModuleType::Output.default_label(), "Out"));

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

        self.volume_meter.paint_stereo(
            &ui.painter().with_clip_rect(rect),
            rect,
            bridge.get_out_volume(),
        );
    }
}

impl GridWidgetContent for OutputWidget {
    fn grid_size(&self) -> GridVec {
        GridVec { x: 2, y: 2 }
    }

    fn show_label(&self) -> bool {
        false
    }

    fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut WidgetCtx, _module_id: ModuleId) {
        self.output_ui(ui, ctx.bridge);
    }
}
