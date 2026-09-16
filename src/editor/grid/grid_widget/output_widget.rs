use egui::{Rect, Vec2, emath::GuiRounding};

use crate::{
    editor::{fit_label::FitLabel, grid::WidgetCtx, volume_meter::VolumeMeter},
    synth_engine::{
        ModuleId, ModuleType, NUM_CHANNELS,
        ui_bridge::{GridVec, UiBridge},
    },
};

use super::GridWidgetContent;

const VERT_PADDING: Vec2 = egui::vec2(4.0, 6.0);
const CLIP_HOLD_SECS: f64 = 1.0;

#[derive(Default)]
pub struct OutputWidget {
    volume_meter: VolumeMeter,
    clipped_until: [Option<f64>; NUM_CHANNELS],
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

        let meter = bridge.get_out_volume();
        let now = ui.input(|i| i.time);
        let mut clipped = [false; NUM_CHANNELS];
        let mut holding_clip = false;

        for (channel_idx, until) in self.clipped_until.iter_mut().enumerate() {
            if meter.clipped[channel_idx] {
                *until = Some(now + CLIP_HOLD_SECS);
            }

            if until.is_some_and(|t| now < t) {
                clipped[channel_idx] = true;
                holding_clip = true;
            } else {
                *until = None;
            }
        }

        if holding_clip {
            ui.ctx().request_repaint();
        }

        self.volume_meter.paint_stereo(
            &ui.painter().with_clip_rect(rect),
            rect,
            meter.volume,
            clipped,
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
