use addi_ui_backend::ui_bridge::modules::mod_map::ModMapUiBridge;
use egui::{Color32, Pos2, Rect, Stroke, StrokeKind, Vec2, emath::GuiRounding, epaint::PathStroke};

use crate::grid::WidgetCtx;
use addi_dsp::segments::Segments;
use addi_engine::mod_map::map_control;
use addi_engine::{Input, ModuleId, Smp};
use addi_ui_backend::ui_bridge::{GridVec, ModuleBridge, UiBridge};

use super::GridWidgetContent;

const PADDING: f32 = 4.0;
const BOTTOM_PADDING: f32 = 2.0;
const LINE_WIDTH: f32 = 2.0;
const GRID_LINE_WIDTH: f32 = 1.0;
const SAMPLES: usize = 256;
const STROKE_COLOR: Color32 = Color32::from_rgb(0x6a, 0xc8, 0x8a);
const GRID_COLOR: Color32 = Color32::from_gray(80);

pub struct ModMapWidget {}

impl ModMapWidget {
    fn map_ui(
        &mut self,
        ui: &mut egui::Ui,
        bridge: &mut UiBridge,
        map_bridge: &mut ModMapUiBridge,
        module_id: ModuleId,
    ) {
        let size = ui.available_size();
        let response = ui.allocate_response(size, egui::Sense::hover());
        let padded = Rect::from_min_max(
            response.rect.min + Vec2::splat(PADDING),
            response.rect.max - Vec2::new(PADDING, PADDING + BOTTOM_PADDING),
        );
        let square = Self::square_in(padded, ui.pixels_per_point());

        if !square.is_positive() || !ui.is_rect_visible(square) {
            return;
        }

        let config = map_bridge.config();
        let mut amount = config.amount;

        bridge.apply_modulation(module_id, Input::Amount, &mut amount);

        let points = Self::curve_points(
            square,
            &config.segments,
            amount[0].clamp(0.0, 1.0),
            config.bipolar,
        );
        let painter = ui.painter();

        Self::paint_grid(painter, square);
        Self::paint_stroke(&painter.with_clip_rect(square.expand(1.0)), &points);
    }

    fn square_in(rect: Rect, ppt: f32) -> Rect {
        let side = rect.width().min(rect.height());
        Rect::from_center_size(rect.center(), Vec2::splat(side)).round_to_pixels(ppt)
    }

    fn paint_grid(painter: &egui::Painter, rect: Rect) {
        let stroke = Stroke::new(GRID_LINE_WIDTH, GRID_COLOR);

        painter.rect_stroke(rect, 0.0, stroke, StrokeKind::Outside);
        painter.line_segment([rect.left_center(), rect.right_center()], stroke);
        painter.line_segment([rect.center_top(), rect.center_bottom()], stroke);
    }

    /// `[0, 1]` ramp after the curve, amount, and bipolar fold. `0` is the bottom when unipolar.
    fn curve_points(rect: Rect, segments: &Segments, amount: Smp, bipolar: bool) -> Vec<Pos2> {
        let t_mult = (SAMPLES - 1) as Smp;

        (0..SAMPLES)
            .map(|i| {
                let x = i as Smp / t_mult;
                let y = map_control(segments.value_at(x, None, None), amount, bipolar);
                let px = rect.left() + x * rect.width();
                let py = if bipolar {
                    rect.center().y - y * rect.height() * 0.5
                } else {
                    rect.bottom() - y * rect.height()
                };

                Pos2::new(px, py)
            })
            .collect()
    }

    fn paint_stroke(painter: &egui::Painter, points: &[Pos2]) {
        if points.len() < 2 {
            return;
        }

        let stroke = PathStroke::new(LINE_WIDTH, STROKE_COLOR).middle();
        painter.line(points.to_vec(), stroke);
    }
}

impl GridWidgetContent for ModMapWidget {
    fn grid_size(&self) -> GridVec {
        GridVec::new(3, 2)
    }

    fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut WidgetCtx, module_id: ModuleId) {
        ctx.bridge
            .with_module_bridge(module_id, |bridge, module_bridge| {
                if let ModuleBridge::ModMap(map_bridge) = module_bridge {
                    self.map_ui(ui, bridge, map_bridge, module_id);
                }
            });
    }
}
