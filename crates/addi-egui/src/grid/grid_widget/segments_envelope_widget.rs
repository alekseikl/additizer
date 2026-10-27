use addi_ui_backend::ui_bridge::modules::segments_envelope::SegmentsEnvelopeUiBridge;
use egui::{Color32, Mesh, Painter, Pos2, Rect, Shape, Stroke, Vec2, ecolor::Hsva};

use crate::grid::WidgetCtx;
use addi_dsp::{Segments, Smp};
use addi_engine::ModuleId;
use addi_engine::segments_envelope::SegmentsEnvelopePhase;
use addi_ui_backend::ui_bridge::ModuleBridge;

use super::GridWidgetContent;

const PADDING: f32 = 4.0;
const PHASE_RADIUS: f32 = 3.5;
/// Yellow. Saturation and value are taken from the left playhead.
const RIGHT_PLAYHEAD_HUE: f32 = 1.0 / 6.0;
const LINE_WIDTH: f32 = 1.0;
const STROKE_COLOR: Color32 = Color32::from_rgb(0x06, 0xaa, 0x1c);
const FILL_ALPHA: u8 = 0x66;
const LOOP_FILL_COLOR: Color32 = Color32::from_rgb(0xff, 0x55, 0x55);

struct RenderedCurve {
    revision: u64,
    size: Vec2,
}

#[derive(Default)]
pub struct SegmentsEnvelopeWidget {
    points: Vec<[Smp; 2]>,
    scratch: Vec<Smp>,
    rendered: Option<RenderedCurve>,
}

impl SegmentsEnvelopeWidget {
    /// Phase/level polyline. Rebuilt when [`Segments::revision`] or `size` changes.
    fn build_curve(&mut self, segments: &Segments, size: Vec2) {
        if size.x <= 0.0 {
            self.points.clear();
            self.rendered = None;
            return;
        }

        let revision = segments.revision();

        if self
            .rendered
            .as_ref()
            .is_some_and(|rendered| rendered.revision == revision && rendered.size == size)
        {
            return;
        }

        let pixels = size.x.round().max(1.0) as usize;

        segments.render_points(pixels, &mut self.points, &mut self.scratch);
        self.rendered = Some(RenderedCurve { revision, size });
    }

    fn phase_pos(rect: Rect, phase: Smp, value: Smp) -> Pos2 {
        Pos2::new(
            rect.left() + phase.clamp(0.0, 1.0) * rect.width(),
            rect.bottom() - value.clamp(0.0, 1.0) * rect.height(),
        )
    }

    fn envelope_ui(
        &mut self,
        ui: &mut egui::Ui,
        envelope_bridge: &mut SegmentsEnvelopeUiBridge,
        has_active_voices: bool,
    ) {
        let size = ui.available_size();
        let response = ui.allocate_response(size, egui::Sense::hover());
        let rect = response.rect.shrink2(egui::vec2(0.0, PADDING));

        if !rect.is_positive() || !ui.is_rect_visible(rect) {
            return;
        }

        self.build_curve(&envelope_bridge.config().segments, rect.size());

        let painter = ui.painter();
        let screen = self
            .points
            .iter()
            .map(|point| Self::phase_pos(rect, point[0], point[1]))
            .collect::<Vec<_>>();

        Self::paint_fill(
            painter,
            rect,
            &screen,
            Self::loop_span(rect, &envelope_bridge.config().segments),
        );
        Self::paint_stroke(painter, &screen);

        if has_active_voices {
            let playhead = envelope_bridge.playhead();

            Self::paint_playheads(painter, rect, &playhead);
        }
    }

    /// Left stays the curve color. The right channel keeps that saturation and value.
    fn paint_playheads(painter: &Painter, rect: Rect, playhead: &SegmentsEnvelopePhase) {
        let left = Self::phase_pos(rect, playhead.phase.left(), playhead.level.left());
        let right = Self::phase_pos(rect, playhead.phase.right(), playhead.level.right());

        if left.distance_sq(right) > 0.25 {
            painter.circle_filled(right, PHASE_RADIUS, Self::right_playhead_color());
        }

        painter.circle_filled(left, PHASE_RADIUS, STROKE_COLOR);
    }

    fn right_playhead_color() -> Color32 {
        let mut color = Hsva::from(STROKE_COLOR);
        color.h = RIGHT_PLAYHEAD_HUE;
        Color32::from(color)
    }

    fn paint_stroke(painter: &Painter, points: &[Pos2]) {
        if points.len() < 2 {
            return;
        }

        painter.line(points.to_vec(), Stroke::new(LINE_WIDTH, STROKE_COLOR));
    }

    /// Screen span of the loop, from the loop point to the release point or the end.
    fn loop_span(rect: Rect, segments: &Segments) -> Option<(f32, f32)> {
        let start = segments.loop_point()?;
        let end_x = segments.release_point().map(|point| point.x).unwrap_or(1.0);
        let left = rect.left() + start.x.clamp(0.0, 1.0) * rect.width();
        let right = rect.left() + end_x.clamp(0.0, 1.0) * rect.width();

        (right > left).then_some((left, right))
    }

    fn paint_fill(painter: &Painter, rect: Rect, points: &[Pos2], loop_span: Option<(f32, f32)>) {
        if points.len() < 2 {
            return;
        }

        let mut mesh = Mesh::default();
        let bottom = rect.bottom();

        for window in points.windows(2) {
            let (start, end) = (window[0], window[1]);

            if end.x <= start.x {
                continue;
            }

            Self::fill_column(&mut mesh, bottom, start, end, loop_span);
        }

        if !mesh.is_empty() {
            painter.add(Shape::mesh(mesh));
        }
    }

    /// One under-curve column, split where it crosses the loop span.
    fn fill_column(
        mesh: &mut Mesh,
        bottom: f32,
        start: Pos2,
        end: Pos2,
        loop_span: Option<(f32, f32)>,
    ) {
        let Some((left, right)) = loop_span else {
            Self::add_quad(mesh, bottom, start, end, STROKE_COLOR);
            return;
        };

        let mut cursor = start;

        if cursor.x < left {
            let next = Self::lerp_point(start, end, left.min(end.x));
            Self::add_quad(mesh, bottom, cursor, next, STROKE_COLOR);
            cursor = next;
        }

        if cursor.x < right && cursor.x < end.x {
            let next = Self::lerp_point(start, end, right.min(end.x));
            Self::add_quad(mesh, bottom, cursor, next, LOOP_FILL_COLOR);
            cursor = next;
        }

        if cursor.x < end.x {
            Self::add_quad(mesh, bottom, cursor, end, STROKE_COLOR);
        }
    }

    fn lerp_point(start: Pos2, end: Pos2, x: f32) -> Pos2 {
        let width = end.x - start.x;

        if width <= 0.0 {
            return end;
        }

        let t = ((x - start.x) / width).clamp(0.0, 1.0);

        Pos2::new(x, start.y + t * (end.y - start.y))
    }

    fn add_quad(mesh: &mut Mesh, bottom: f32, start: Pos2, end: Pos2, color: Color32) {
        if end.x <= start.x {
            return;
        }

        let fill = Self::with_alpha(color, FILL_ALPHA);
        let index = mesh.vertices.len() as u32;

        mesh.colored_vertex(start, fill);
        mesh.colored_vertex(end, fill);
        mesh.colored_vertex(Pos2::new(end.x, bottom), fill);
        mesh.colored_vertex(Pos2::new(start.x, bottom), fill);
        mesh.add_triangle(index, index + 1, index + 2);
        mesh.add_triangle(index, index + 2, index + 3);
    }

    fn with_alpha(color: Color32, alpha: u8) -> Color32 {
        Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha)
    }
}

impl GridWidgetContent for SegmentsEnvelopeWidget {
    fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut WidgetCtx, module_id: ModuleId) {
        let has_active_voices = ctx.bridge.has_active_voices();

        ctx.bridge
            .with_module_bridge(module_id, |_bridge, module_bridge| {
                if let ModuleBridge::SegmentsEnvelope(envelope_bridge) = module_bridge {
                    self.envelope_ui(ui, envelope_bridge, has_active_voices);
                }
            });
    }
}

#[cfg(test)]
mod tests;
