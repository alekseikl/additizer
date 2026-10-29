use addi_ui_backend::ui_bridge::modules::mod_map::ModMapUiBridge;
use egui::{
    Color32, Painter, Pos2, Rect, Stroke, StrokeKind, Vec2, ecolor::Hsva, emath::GuiRounding,
    epaint::PathStroke,
};

use crate::fit_label::FitLabel;
use crate::grid::WidgetCtx;
use crate::routing_ui_ext::ModuleTypeUi;
use addi_dsp::segments::Segments;
use addi_engine::{Input, ModuleId, ModuleType, Smp, StereoSample};
use addi_ui_backend::ui_bridge::{GridVec, ModuleBridge, UiBridge};

use super::GridWidgetContent;

const VERT_PADDING: f32 = 2.0;
const PROBE_RADIUS: f32 = 3.5;
/// Yellow. Saturation and value are taken from the left probe.
const RIGHT_PROBE_HUE: f32 = 1.0 / 6.0;
const LINE_WIDTH: f32 = 1.0;
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
        has_active_voices: bool,
    ) {
        ui.add_space(2.0);
        ui.add(FitLabel::new(ModuleType::ModMap.default_label(), "Map"));

        let size = ui.available_size();
        let response = ui.allocate_response(size, egui::Sense::hover());
        let inner = Rect::from_min_max(
            response.rect.min + Vec2::new(0.0, VERT_PADDING),
            response.rect.max - Vec2::new(0.0, VERT_PADDING),
        );
        let plot = Self::square_in(inner, ui.pixels_per_point());

        if !plot.is_positive() || !ui.is_rect_visible(plot) {
            return;
        }

        let probe = map_bridge.probe();
        let config = map_bridge.config();
        let mut amount = config.amount;

        bridge.apply_modulation(module_id, Input::Amount, &mut amount);

        let amount = amount[0].clamp(0.0, 1.0);
        let points = Self::curve_points(plot, &config.segments, amount, config.bipolar);
        let painter = ui.painter();

        Self::paint_grid(painter, plot, config.bipolar);
        Self::paint_stroke(&painter.with_clip_rect(plot), &points);

        if has_active_voices {
            Self::paint_probe(
                painter,
                plot,
                &config.segments,
                amount,
                config.bipolar,
                probe,
            );
        }
    }

    fn value_pos(rect: Rect, x: Smp, y: Smp, bipolar: bool) -> Pos2 {
        let y = if bipolar {
            (y.clamp(-1.0, 1.0) + 1.0) * 0.5
        } else {
            y.clamp(0.0, 1.0)
        };

        Pos2::new(
            rect.left() + x.clamp(0.0, 1.0) * rect.width(),
            rect.bottom() - y * rect.height(),
        )
    }

    /// Left stays the curve color. The right channel keeps that saturation and value.
    fn paint_probe(
        painter: &Painter,
        rect: Rect,
        segments: &Segments,
        amount: Smp,
        bipolar: bool,
        probe: StereoSample,
    ) {
        let at = |x: Smp| {
            Self::value_pos(
                rect,
                x,
                Self::map_control(segments.value_at(x, None, None), amount, bipolar),
                bipolar,
            )
        };
        let left = at(probe.left());
        let right = at(probe.right());

        if left.distance_sq(right) > 0.25 {
            painter.circle_filled(right, PROBE_RADIUS, Self::right_probe_color());
        }

        painter.circle_filled(left, PROBE_RADIUS, STROKE_COLOR);
    }

    fn right_probe_color() -> Color32 {
        let mut color = Hsva::from(STROKE_COLOR);
        color.h = RIGHT_PROBE_HUE;
        Color32::from(color)
    }

    fn square_in(rect: Rect, ppt: f32) -> Rect {
        let side = rect.width().min(rect.height());
        Rect::from_center_size(rect.center(), Vec2::splat(side)).round_to_pixels(ppt)
    }

    fn paint_grid(painter: &egui::Painter, rect: Rect, bipolar: bool) {
        let stroke = Stroke::new(GRID_LINE_WIDTH, GRID_COLOR);

        painter.rect_stroke(rect, 0.0, stroke, StrokeKind::Inside);

        if bipolar {
            painter.line_segment([rect.left_center(), rect.right_center()], stroke);
        }
    }

    /// Fold into `[-1, 1]` when bipolar, then scale by amount.
    fn map_control(mapped: Smp, amount: Smp, bipolar: bool) -> Smp {
        let mapped = if bipolar {
            mapped.mul_add(2.0, -1.0)
        } else {
            mapped
        };

        mapped * amount.clamp(0.0, 1.0)
    }

    /// Curve after the bipolar fold and the amount scale. Unipolar `0` is the bottom; bipolar `0` is the midline.
    fn curve_points(rect: Rect, segments: &Segments, amount: Smp, bipolar: bool) -> Vec<Pos2> {
        let t_mult = (SAMPLES - 1) as Smp;

        (0..SAMPLES)
            .map(|i| {
                let x = i as Smp / t_mult;
                let y = Self::map_control(segments.value_at(x, None, None), amount, bipolar);

                Self::value_pos(rect, x, y, bipolar)
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
        GridVec::new(2, 2)
    }

    fn show_label(&self) -> bool {
        false
    }

    fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut WidgetCtx, module_id: ModuleId) {
        let has_active_voices = ctx.bridge.has_active_voices();

        ctx.bridge
            .with_module_bridge(module_id, |bridge, module_bridge| {
                if let ModuleBridge::ModMap(map_bridge) = module_bridge {
                    self.map_ui(ui, bridge, map_bridge, module_id, has_active_voices);
                }
            });
    }
}

#[cfg(test)]
mod tests;
