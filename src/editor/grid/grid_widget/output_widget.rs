use crate::{
    editor::{
        grid::WidgetCtx,
        stereo_smoother::StereoSmoother,
        volume_meter,
    },
    synth_engine::{
        ModuleId, Sample, StereoSample,
        ui_bridge::{GridVec, UiBridge},
    },
};

use super::GridWidgetContent;

const PADDING: f32 = 8.0;
const VOLUME_SMOOTH_TIME: Sample = 0.15;

pub struct OutputWidget {
    volume_smoother: StereoSmoother,
}

impl Default for OutputWidget {
    fn default() -> Self {
        Self {
            volume_smoother: StereoSmoother::new(StereoSample::ZERO, VOLUME_SMOOTH_TIME),
        }
    }
}

impl OutputWidget {
    fn output_ui(&mut self, ui: &mut egui::Ui, bridge: &mut UiBridge) {
        let size = ui.available_size();
        let response = ui.allocate_response(size, egui::Sense::hover());
        let rect = response.rect.shrink2(egui::vec2(0.0, PADDING));

        if !rect.is_positive() || !ui.is_rect_visible(rect) {
            return;
        }

        let volume = self.volume_smoother.tick(bridge.get_out_volume());

        volume_meter::paint_stereo(&ui.painter().with_clip_rect(rect), rect, volume);
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
