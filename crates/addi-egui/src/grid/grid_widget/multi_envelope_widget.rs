use addi_ui_backend::ui_bridge::modules::multi_envelope::MultiEnvelopeUiBridge;
use egui::{Color32, Mesh, Painter, Pos2, Rect, Shape, Stroke, Vec2, ecolor::Hsva};

use crate::grid::WidgetCtx;
use crate::utils::hsva;
use addi_dsp::Smp;
use addi_dsp::segments::time_segments::TimeSegments;
use addi_engine::ModuleId;
use addi_engine::multi_envelope::MultiEnvelopePhase;
use addi_ui_backend::ui_bridge::ModuleBridge;

use super::GridWidgetContent;

/// Seconds across the tile. The detail editor starts here, then follows scroll and drag.
const SPAN: Smp = 4.0;
const PADDING: f32 = 4.0;
const PHASE_RADIUS: f32 = 3.5;
const RIGHT_PLAYHEAD_HUE: f32 = 1.0 / 6.0;
const LINE_WIDTH: f32 = 1.0;
const STROKE_COLOR: Color32 = Color32::from_rgb(0x06, 0xaa, 0x1c);
const FILL_ALPHA: u8 = 0x66;
const LOOP_FILL_COLOR: Hsva = hsva(0.0, 0.9092, 1.0, 0.25);

struct RenderedCurve {
    revision: u64,
    size: Vec2,
    span_bits: u32,
}

#[derive(Default)]
pub struct MultiEnvelopeWidget {
    points: Vec<[Smp; 2]>,
    scratch: Vec<Smp>,
    rendered: Option<RenderedCurve>,
}

impl MultiEnvelopeWidget {
    fn build_curve(&mut self, segments: &TimeSegments, size: Vec2, span: Smp) {
        if size.x <= 0.0 {
            self.points.clear();
            self.rendered = None;
            return;
        }

        let revision = segments.revision();
        let span_bits = span.to_bits();

        if self.rendered.as_ref().is_some_and(|rendered| {
            rendered.revision == revision
                && rendered.size == size
                && rendered.span_bits == span_bits
        }) {
            return;
        }

        let pixels = size.x.round().max(1.0) as usize;

        segments.render_points(pixels, span, &mut self.points, &mut self.scratch);
        self.rendered = Some(RenderedCurve {
            revision,
            size,
            span_bits,
        });
    }

    fn time_pos(rect: Rect, time: Smp, value: Smp, span: Smp) -> Pos2 {
        let x = if span > 0.0 { time / span } else { 0.0 };

        Pos2::new(
            rect.left() + x.clamp(0.0, 1.0) * rect.width(),
            rect.bottom() - value.clamp(0.0, 1.0) * rect.height(),
        )
    }

    fn envelope_ui(
        &mut self,
        ui: &mut egui::Ui,
        envelope_bridge: &mut MultiEnvelopeUiBridge,
        has_active_voices: bool,
    ) {
        let size = ui.available_size();
        let response = ui.allocate_response(size, egui::Sense::hover());
        let rect = response.rect.shrink2(egui::vec2(0.0, PADDING));

        if !rect.is_positive() || !ui.is_rect_visible(rect) {
            return;
        }

        let segments = &envelope_bridge.config().segments;
        let span = SPAN;

        self.build_curve(segments, rect.size(), span);

        let painter = ui.painter();
        let screen = self
            .points
            .iter()
            .map(|point| Self::time_pos(rect, point[0], point[1], span))
            .collect::<Vec<_>>();

        Self::paint_fill(
            painter,
            rect,
            &screen,
            Self::loop_span(rect, segments, span),
        );
        Self::paint_stroke(painter, &screen);

        if has_active_voices {
            let playhead = envelope_bridge.playhead();

            Self::paint_playheads(painter, rect, &playhead, span);
        }
    }

    fn paint_playheads(painter: &Painter, rect: Rect, playhead: &MultiEnvelopePhase, span: Smp) {
        let left = Self::time_pos(rect, playhead.time.left(), playhead.level.left(), span);
        let right = Self::time_pos(rect, playhead.time.right(), playhead.level.right(), span);

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

    fn loop_span(rect: Rect, segments: &TimeSegments, span: Smp) -> Option<(f32, f32)> {
        let start = segments.loop_point_idx()?;
        let end_time = segments
            .release_point_idx()
            .map(|index| segments.time(index))
            .unwrap_or_else(|| segments.total_time());
        let left = Self::time_pos(rect, segments.time(start), 0.0, span).x;
        let right = Self::time_pos(rect, end_time, 0.0, span).x;

        (right > left).then_some((left, right))
    }

    fn paint_fill(painter: &Painter, rect: Rect, points: &[Pos2], loop_span: Option<(f32, f32)>) {
        if points.len() >= 2 {
            let mut mesh = Mesh::default();
            let bottom = rect.bottom();

            for &[start, end] in points.array_windows() {
                if end.x <= start.x {
                    continue;
                }

                Self::add_quad(&mut mesh, bottom, start, end, STROKE_COLOR);
            }

            if !mesh.is_empty() {
                painter.add(Shape::mesh(mesh));
            }
        }

        let Some((left, right)) = loop_span else {
            return;
        };
        let band = Rect::from_x_y_ranges(left..=right, rect.y_range());

        painter.rect_filled(band, 0.0, LOOP_FILL_COLOR);
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

impl GridWidgetContent for MultiEnvelopeWidget {
    fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut WidgetCtx, module_id: ModuleId) {
        let has_active_voices = ctx.bridge.has_active_voices();

        ctx.bridge
            .with_module_bridge(module_id, |_bridge, module_bridge| {
                if let ModuleBridge::MultiEnvelope(envelope_bridge) = module_bridge {
                    self.envelope_ui(ui, envelope_bridge, has_active_voices);
                }
            });
    }
}
