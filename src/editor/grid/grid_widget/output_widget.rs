use crate::{
    editor::{grid::WidgetCtx, volume_meter::VolumeMeter},
    synth_engine::{
        ModuleId,
        ui_bridge::{GridVec, UiBridge},
    },
};

use super::GridWidgetContent;

const PADDING: f32 = 8.0;

#[derive(Default)]
pub struct OutputWidget {
    volume_meter: VolumeMeter,
}

impl OutputWidget {
    fn output_ui(&mut self, ui: &mut egui::Ui, bridge: &mut UiBridge) {
        let size = ui.available_size();
        let response = ui.allocate_response(size, egui::Sense::hover());
        let rect = response.rect.shrink2(egui::vec2(0.0, PADDING));

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
