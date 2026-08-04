use egui::{Color32, Painter, Pos2, Rect, Vec2, emath::GuiRounding};

use crate::{
    editor::{fit_label::FitLabel, grid::WidgetCtx},
    synth_engine::{
        ModuleId, Sample,
        external_param::ExternalParamUiBridge,
        ui_bridge::{GridVec, ModuleBridge},
    },
};

use super::GridWidgetContent;

const VERT_PADDING: Vec2 = egui::vec2(4.0, 6.0);
const NUM_SEGMENTS: usize = 12;
const SEGMENT_GAP: f32 = 2.0;
const BAR_MAX_WIDTH: f32 = 24.0;
const BAR_MIN_WIDTH: f32 = 4.0;

const OFF_COLOR: Color32 = Color32::from_rgb(36, 38, 50);
const GREEN: Color32 = Color32::from_rgb(0x06, 0xaa, 0x1c);

#[derive(Default)]
pub struct ExternalParamWidget {}

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

        let value = param_bridge.get_value().clamp(0.0, 1.0);
        Self::paint_segments(&ui.painter().with_clip_rect(rect), rect, value);
    }

    fn paint_segments(painter: &Painter, rect: Rect, value: Sample) {
        let bar_width = rect.width().clamp(BAR_MIN_WIDTH, BAR_MAX_WIDTH);
        let left = rect.center().x - bar_width * 0.5;
        let bar_rect = Rect::from_min_max(
            Pos2::new(left, rect.top()),
            Pos2::new(left + bar_width, rect.bottom()),
        );

        let segment_height =
            (bar_rect.height() - SEGMENT_GAP * (NUM_SEGMENTS - 1) as f32) / NUM_SEGMENTS as f32;

        if segment_height <= 0.0 {
            return;
        }

        for segment_idx in 0..NUM_SEGMENTS {
            let bottom = bar_rect.bottom() - segment_idx as f32 * (segment_height + SEGMENT_GAP);
            let segment_rect = Rect::from_min_max(
                Pos2::new(bar_rect.left(), bottom - segment_height),
                Pos2::new(bar_rect.right(), bottom),
            );
            let brightness = Self::segment_brightness(segment_idx, value);
            let color = if brightness <= 0.0 {
                OFF_COLOR
            } else {
                OFF_COLOR.lerp_to_gamma(GREEN, brightness)
            };

            painter.rect_filled(segment_rect, 0.0, color);
        }
    }

    fn segment_brightness(segment_idx: usize, value: Sample) -> Sample {
        let lower = segment_idx as Sample / NUM_SEGMENTS as Sample;
        let upper = (segment_idx + 1) as Sample / NUM_SEGMENTS as Sample;

        if value <= lower {
            0.0
        } else if value >= upper {
            1.0
        } else {
            (value - lower) / (upper - lower)
        }
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
