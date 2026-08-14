use egui::{Color32, Pos2, Rect, Stroke, StrokeKind, Vec2, emath::GuiRounding, epaint::PathStroke};

use crate::{
    editor::grid::WidgetCtx,
    synth_engine::{
        Input, ModuleId, Sample, ShaperType,
        ui_bridge::{GridVec, ModuleBridge, UiBridge},
        wave_shaper::WaveShaperUiBridge,
    },
    utils::db_to_gain_fast,
};

use super::GridWidgetContent;

const PADDING: f32 = 4.0;
const BOTTOM_PADDING: f32 = 2.0;
const LINE_WIDTH: f32 = 2.0;
const GRID_LINE_WIDTH: f32 = 1.0;
const SAMPLES: usize = 256;
const STROKE_COLOR: Color32 = Color32::from_rgb(0xe0, 0x6a, 0x6a);
const GRID_COLOR: Color32 = Color32::from_gray(80);

pub struct WaveShaperWidget {}

impl WaveShaperWidget {
    fn shaper_ui(
        &mut self,
        ui: &mut egui::Ui,
        bridge: &mut UiBridge,
        shaper_bridge: &mut WaveShaperUiBridge,
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

        let mut config = shaper_bridge.config().clone();

        bridge.apply_modulation(module_id, Input::Distortion, &mut config.distortion);
        bridge.apply_modulation(module_id, Input::ClippingLevel, &mut config.clipping_level);

        let gain = db_to_gain_fast(config.distortion[0]);
        let clipping_gain = db_to_gain_fast(config.clipping_level[0]);
        let points = Self::curve_points(square, config.shaper_type, gain, clipping_gain);
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

    fn curve_points(
        rect: Rect,
        shaper: ShaperType,
        gain: Sample,
        clipping_gain: Sample,
    ) -> Vec<Pos2> {
        let t_mult = (SAMPLES - 1) as Sample;
        let half_w = rect.width() * 0.5;
        let half_h = rect.height() * 0.5;
        let center = rect.center();

        (0..SAMPLES)
            .map(|i| {
                let t = i as Sample / t_mult;
                let x = t.mul_add(2.0, -1.0);
                let y = shaper.apply(x, gain, clipping_gain);

                Pos2::new(center.x + x * half_w, center.y - y * half_h)
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

impl GridWidgetContent for WaveShaperWidget {
    fn grid_size(&self) -> GridVec {
        GridVec::new(3, 2)
    }

    fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut WidgetCtx, module_id: ModuleId) {
        ctx.bridge
            .with_module_bridge(module_id, |bridge, module_bridge| {
                if let ModuleBridge::WaveShaper(shaper_bridge) = module_bridge {
                    self.shaper_ui(ui, bridge, shaper_bridge, module_id);
                }
            });
    }
}
