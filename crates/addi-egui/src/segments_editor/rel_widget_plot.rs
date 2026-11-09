use egui::{Color32, Painter, Pos2, Rect, Stroke, Ui, Vec2, ecolor::Hsva, pos2};

use addi_dsp::{RelSegments, Smp};

use crate::utils::{FillTexture, hsva};

use super::common;

const LINE_WIDTH: f32 = 1.0;
const PLAYHEAD_RADIUS: f32 = 3.5;
const FILL_TOP_ALPHA: u8 = 0x80;
const FILL_BOTTOM_ALPHA: u8 = 0x60;
const LFO_WAVE: Color32 = Color32::from_rgb(0x4a, 0xb0, 0xff);
const ENVELOPE_WAVE: Color32 = Color32::from_rgb(0x06, 0xaa, 0x1c);
const LOOP_FILL: Hsva = hsva(0.0, 0.9092, 1.0, 0.25);

struct Cache {
    revision: u64,
    size: Vec2,
}

#[derive(Clone, Copy)]
pub struct CurveMap {
    pub amount: Smp,
    pub bipolar: bool,
}

pub struct RelWidgetPlot {
    curve: Vec<[Smp; 2]>,
    scratch: Vec<Smp>,
    cache: Option<Cache>,
    wave: Color32,
    fill: FillTexture,
}

impl RelWidgetPlot {
    /// Blue phase curve and a gradient fill.
    pub fn lfo() -> Self {
        Self::new(LFO_WAVE)
    }

    /// Green phase curve and a gradient fill.
    pub fn envelope() -> Self {
        Self::new(ENVELOPE_WAVE)
    }

    /// Mapped curve and a gradient fill.
    pub fn map() -> Self {
        Self::new(LFO_WAVE)
    }

    fn new(wave: Color32) -> Self {
        Self {
            curve: Vec::new(),
            scratch: Vec::new(),
            cache: None,
            wave,
            fill: FillTexture::with_alpha_range(wave, FILL_TOP_ALPHA, FILL_BOTTOM_ALPHA),
        }
    }

    /// `playhead` is the left and right point, each `(phase, level)` before `map`.
    pub fn paint(
        &mut self,
        ui: &Ui,
        plot: Rect,
        segments: &RelSegments,
        map: Option<CurveMap>,
        playhead: Option<[(Smp, Smp); 2]>,
    ) {
        if !plot.is_positive() {
            return;
        }

        self.build_curve(segments, plot.size());

        let screen_points = self.project(plot, map);

        let painter = ui.painter();
        let bipolar = map.is_some_and(|map| map.bipolar);
        let baseline = if bipolar {
            plot.center().y
        } else {
            plot.bottom()
        };

        common::paint_fill(
            painter,
            plot,
            self.fill.id(ui.ctx()),
            &screen_points,
            baseline,
        );

        if let Some(band) = loop_band(plot, segments) {
            painter.rect_filled(band, 0.0, LOOP_FILL);
        }

        painter.line(screen_points, Stroke::new(LINE_WIDTH, self.wave));

        if let Some([left, right]) = playhead {
            paint_playheads(
                painter,
                screen_pos(plot, left.0, level(left.1, map), bipolar),
                screen_pos(plot, right.0, level(right.1, map), bipolar),
                self.wave,
            );
        }
    }

    /// Phase/level polyline. Rebuilt when [`RelSegments::revision`] or `size` changes.
    fn build_curve(&mut self, segments: &RelSegments, size: Vec2) {
        assert!(size.x > 0.0);

        let revision = segments.revision();

        if self
            .cache
            .as_ref()
            .is_some_and(|cache| cache.revision == revision && cache.size == size)
        {
            return;
        }

        let pixels = size.x.round().max(1.0) as usize;

        segments.render_points(pixels, &mut self.curve, &mut self.scratch);
        self.cache = Some(Cache { revision, size });
    }

    fn project(&self, plot: Rect, map: Option<CurveMap>) -> Vec<Pos2> {
        let bipolar = map.is_some_and(|map| map.bipolar);

        self.curve
            .iter()
            .copied()
            .map(|[x, y]| screen_pos(plot, x, level(y, map), bipolar))
            .collect()
    }
}

/// Fold into `[-1, 1]` when bipolar, then scale by amount. No map leaves the level unchanged.
fn level(y: Smp, map: Option<CurveMap>) -> Smp {
    let Some(map) = map else {
        return y;
    };

    let y = if map.bipolar { y.mul_add(2.0, -1.0) } else { y };

    y * map.amount.clamp(0.0, 1.0)
}

fn screen_pos(plot: Rect, x: Smp, y: Smp, bipolar: bool) -> Pos2 {
    let y = if bipolar {
        (y.clamp(-1.0, 1.0) + 1.0) * 0.5
    } else {
        y.clamp(0.0, 1.0)
    };

    pos2(
        plot.left() + x.clamp(0.0, 1.0) * plot.width(),
        plot.bottom() - y * plot.height(),
    )
}

fn screen_x(plot: Rect, x: Smp) -> f32 {
    plot.left() + x.clamp(0.0, 1.0) * plot.width()
}

/// Screen span of the loop, from the loop point to the release point or the end.
fn loop_band(plot: Rect, segments: &RelSegments) -> Option<Rect> {
    let start = segments.loop_point()?;
    let end_x = segments.release_point().map(|point| point.x).unwrap_or(1.0);

    common::horizontal_span(plot, screen_x(plot, start.x), screen_x(plot, end_x))
}

/// Left stays `wave`. The right channel keeps that saturation and value.
fn paint_playheads(painter: &Painter, left: Pos2, right: Pos2, wave: Color32) {
    if left.distance_sq(right) > common::PLAYHEAD_OVERLAP_SQ {
        let mut color = Hsva::from(wave);

        color.h = common::RIGHT_PLAYHEAD_HUE;
        painter.circle_filled(right, PLAYHEAD_RADIUS, color);
    }

    painter.circle_filled(left, PLAYHEAD_RADIUS, wave);
}

#[cfg(test)]
mod tests;
