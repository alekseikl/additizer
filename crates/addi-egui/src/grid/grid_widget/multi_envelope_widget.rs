use addi_ui_backend::ui_bridge::modules::multi_envelope::MultiEnvelopeUiBridge;
use egui::{Color32, Painter, Pos2, Rect, Stroke, Vec2, ecolor::Hsva};

use crate::grid::WidgetCtx;
use crate::segments_editor::{PLAYHEAD_OVERLAP_SQ, RIGHT_PLAYHEAD_HUE, horizontal_span, paint_fill};
use crate::utils::{FillTexture, hsva};
use addi_dsp::Smp;
use addi_dsp::segments::time_segments::TimeSegments;
use addi_engine::ModuleId;
use addi_engine::multi_envelope::MultiEnvelopePhase;
use addi_ui_backend::ui_bridge::ModuleBridge;

use super::GridWidgetContent;

const PADDING: f32 = 4.0;
const LINE_WIDTH: f32 = 1.0;
const PLAYHEAD_RADIUS: f32 = 3.5;
const FILL_TOP_ALPHA: u8 = 0x80;
const FILL_BOTTOM_ALPHA: u8 = 0x60;
const ENVELOPE_WAVE: Color32 = Color32::from_rgb(0x06, 0xaa, 0x1c);
const LOOP_FILL: Hsva = hsva(0.0, 0.9092, 1.0, 0.25);

struct RenderedCurve {
    revision: u64,
    size: Vec2,
    span_bits: u32,
}

pub struct MultiEnvelopeWidget {
    points: Vec<[Smp; 2]>,
    scratch: Vec<Smp>,
    rendered: Option<RenderedCurve>,
    fill: FillTexture,
}

impl Default for MultiEnvelopeWidget {
    fn default() -> Self {
        Self {
            points: Vec::new(),
            scratch: Vec::new(),
            rendered: None,
            fill: FillTexture::with_alpha_range(ENVELOPE_WAVE, FILL_TOP_ALPHA, FILL_BOTTOM_ALPHA),
        }
    }
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
        let span = segments.total_time().max(Smp::EPSILON);

        self.build_curve(segments, rect.size(), span);

        let painter = ui.painter();
        let screen = self
            .points
            .iter()
            .map(|point| Self::time_pos(rect, point[0], point[1], span))
            .collect::<Vec<_>>();

        paint_fill(painter, rect, self.fill.id(ui.ctx()), &screen, rect.bottom());

        if let Some(band) = Self::loop_band(rect, segments, span) {
            painter.rect_filled(band, 0.0, LOOP_FILL);
        }

        painter.line(screen, Stroke::new(LINE_WIDTH, ENVELOPE_WAVE));

        if has_active_voices {
            let playhead = envelope_bridge.playhead();

            Self::paint_playheads(painter, rect, &playhead, span);
        }
    }

    fn paint_playheads(painter: &Painter, rect: Rect, playhead: &MultiEnvelopePhase, span: Smp) {
        let left = Self::time_pos(rect, playhead.time.left(), playhead.level.left(), span);
        let right = Self::time_pos(rect, playhead.time.right(), playhead.level.right(), span);

        if left.distance_sq(right) > PLAYHEAD_OVERLAP_SQ {
            let mut color = Hsva::from(ENVELOPE_WAVE);

            color.h = RIGHT_PLAYHEAD_HUE;
            painter.circle_filled(right, PLAYHEAD_RADIUS, color);
        }

        painter.circle_filled(left, PLAYHEAD_RADIUS, ENVELOPE_WAVE);
    }

    fn loop_band(rect: Rect, segments: &TimeSegments, span: Smp) -> Option<Rect> {
        let start = segments.loop_point_idx()?;
        let end_time = segments
            .release_point_idx()
            .map(|index| segments.time(index))
            .unwrap_or_else(|| segments.total_time());

        horizontal_span(
            rect,
            Self::time_pos(rect, segments.time(start), 0.0, span).x,
            Self::time_pos(rect, end_time, 0.0, span).x,
        )
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
