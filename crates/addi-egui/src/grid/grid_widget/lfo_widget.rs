use addi_ui_backend::ui_bridge::modules::lfo::LfoUiBridge;
use egui::{Color32, Mesh, Painter, Pos2, Rect, Shape, Stroke};

use crate::grid::WidgetCtx;
use addi_dsp::{CurvePoint, Segments, Smp};
use addi_engine::ModuleId;
use addi_ui_backend::ui_bridge::ModuleBridge;

use super::GridWidgetContent;

const PADDING: f32 = 4.0;
const PHASE_RADIUS: f32 = 3.5;
const LINE_WIDTH: f32 = 1.0;
const FILL_EDGE_ALPHA: u8 = 0x47;
const FILL_BASE_ALPHA: u8 = 0x66;
const WAVE_COLOR: Color32 = Color32::from_rgb(0x4a, 0xb0, 0xff);

#[derive(Default)]
pub struct LfoWidget {
    points: Vec<CurvePoint>,
    scratch: Vec<Smp>,
}

impl LfoWidget {
    fn build_curve(&mut self, segments: &Segments, pixels: usize) {
        segments.render_points(pixels, &mut self.points, &mut self.scratch);
    }

    fn phase_pos(rect: Rect, phase: Smp, value: Smp) -> Pos2 {
        Pos2::new(
            rect.left() + phase.clamp(0.0, 1.0) * rect.width(),
            rect.bottom() - value.clamp(0.0, 1.0) * rect.height(),
        )
    }

    fn lfo_ui(&mut self, ui: &mut egui::Ui, lfo_bridge: &mut LfoUiBridge, has_active_voices: bool) {
        let size = ui.available_size();
        let response = ui.allocate_response(size, egui::Sense::hover());
        let rect = response.rect.shrink2(egui::vec2(0.0, PADDING));

        if !rect.is_positive() || !ui.is_rect_visible(rect) {
            return;
        }

        let pixels = rect.width().round().max(1.0) as usize;

        self.build_curve(&lfo_bridge.config().segments, pixels);

        let painter = ui.painter();
        let screen = self
            .points
            .iter()
            .map(|point| Self::phase_pos(rect, point.x, point.y))
            .collect::<Vec<_>>();

        Self::paint_fill(painter, rect, &screen);
        Self::paint_stroke(painter, &screen);

        if has_active_voices {
            let phase = lfo_bridge.get_phase();
            let level = lfo_bridge.config().segments.value_at(phase);

            painter.circle_filled(
                Self::phase_pos(rect, phase, level),
                PHASE_RADIUS,
                WAVE_COLOR,
            );
        }
    }

    fn paint_stroke(painter: &Painter, points: &[Pos2]) {
        if points.len() < 2 {
            return;
        }

        painter.line(points.to_vec(), Stroke::new(LINE_WIDTH, WAVE_COLOR));
    }

    fn paint_fill(painter: &Painter, rect: Rect, points: &[Pos2]) {
        if points.len() < 2 {
            return;
        }

        let mut mesh = Mesh::default();
        let edge = Self::with_alpha(WAVE_COLOR, FILL_EDGE_ALPHA);
        let base = Self::with_alpha(WAVE_COLOR, FILL_BASE_ALPHA);
        let bottom = rect.bottom();

        for window in points.windows(2) {
            let (start, end) = (window[0], window[1]);

            if end.x <= start.x {
                continue;
            }

            let index = mesh.vertices.len() as u32;

            mesh.colored_vertex(start, edge);
            mesh.colored_vertex(end, edge);
            mesh.colored_vertex(Pos2::new(end.x, bottom), base);
            mesh.colored_vertex(Pos2::new(start.x, bottom), base);
            mesh.add_triangle(index, index + 1, index + 2);
            mesh.add_triangle(index, index + 2, index + 3);
        }

        if !mesh.is_empty() {
            painter.add(Shape::mesh(mesh));
        }
    }

    fn with_alpha(color: Color32, alpha: u8) -> Color32 {
        Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha)
    }
}

impl GridWidgetContent for LfoWidget {
    fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut WidgetCtx, module_id: ModuleId) {
        let has_active_voices = ctx.bridge.has_active_voices();

        ctx.bridge
            .with_module_bridge(module_id, |_bridge, module_bridge| {
                if let ModuleBridge::Lfo(lfo_bridge) = module_bridge {
                    self.lfo_ui(ui, lfo_bridge, has_active_voices);
                }
            });
    }
}

#[cfg(test)]
mod tests;
