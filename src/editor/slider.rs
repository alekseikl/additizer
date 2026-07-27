use std::ops::RangeInclusive;

use egui::{
    Color32, FontFamily, FontId, Id, LayerId, Mesh, Order, Painter, PointerButton, Pos2, Rect,
    Response, Sense, Shape, Stroke, StrokeKind, TextureId, Ui, Widget, ecolor::Hsva,
    epaint::PathStroke, epaint::Vertex, vec2,
};

use crate::synth_engine::{Sample, StereoSample};

const fn hsva(h: f32, s: f32, v: f32, a: f32) -> Hsva {
    Hsva { h, s, v, a }
}

const BG_COLOR: Hsva = hsva(0.115, 0.05, 0.01, 1.0);
const BORDER_COLOR: Hsva = hsva(0.115, 0.35, 0.2, 1.0);

const LEVEL_COLOR: Hsva = hsva(0.1, 0.8, 0.15, 1.0);
const LEVEL_TIP_COLOR: Hsva = hsva(0.1, 0.8, 0.5, 1.0);

const INVERSE_LEVEL_COLOR: Hsva = hsva(0.06, 0.8, 0.15, 1.0);
const INVERSE_LEVEL_TIP_COLOR: Hsva = hsva(0.06, 0.8, 0.5, 1.0);

const OVER_COLOR: Hsva = hsva(0.03, 0.8, 0.15, 1.0);
const OVER_TIP_COLOR: Hsva = hsva(0.03, 0.8, 0.5, 1.0);

const CORNER_RADIUS: f32 = 4.0;

/// Width of the bright leading tip marker at the fill's edge.
const TIP_WIDTH: f32 = 1.0;

const LABEL_TEXT_COLOR: Hsva = hsva(0.115, 0.05, 0.5, 1.0);
const LABEL_PADDING: f32 = 6.0;
const LABEL_MARGIN: f32 = 1.0;
const HOVER_DELAY: f64 = 0.5;

/// Track orientation.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum Orientation {
    #[default]
    Horizontal,
    Vertical,
}

impl Orientation {
    /// Screen coordinate along the track at track position `t` (0 = start end, span = far end).
    fn along_screen(self, rect: Rect, t: f32) -> f32 {
        match self {
            Self::Horizontal => rect.left() + t,
            Self::Vertical => rect.bottom() - t,
        }
    }

    fn along_span(self, rect: Rect) -> f32 {
        match self {
            Self::Horizontal => rect.width(),
            Self::Vertical => rect.height(),
        }
    }

    fn dist_from_start(self, rect: Rect, along_screen: f32) -> f32 {
        match self {
            Self::Horizontal => along_screen - rect.left(),
            Self::Vertical => rect.bottom() - along_screen,
        }
    }

    fn dist_from_end(self, rect: Rect, along_screen: f32) -> f32 {
        match self {
            Self::Horizontal => rect.right() - along_screen,
            Self::Vertical => along_screen - rect.top(),
        }
    }

    fn across_low(self, rect: Rect) -> f32 {
        match self {
            Self::Horizontal => rect.top(),
            Self::Vertical => rect.left(),
        }
    }

    fn across_high(self, rect: Rect) -> f32 {
        match self {
            Self::Horizontal => rect.bottom(),
            Self::Vertical => rect.right(),
        }
    }

    fn point(self, along_screen: f32, across: f32) -> Pos2 {
        match self {
            Self::Horizontal => Pos2::new(along_screen, across),
            Self::Vertical => Pos2::new(across, along_screen),
        }
    }
}

/// Which across-axis edges of a bar have rounded corners.
#[derive(Clone, Copy)]
struct RoundedEdges {
    low: bool,
    high: bool,
}

impl RoundedEdges {
    const ALL: Self = Self {
        low: true,
        high: true,
    };

    const FIRST: Self = Self {
        low: true,
        high: false,
    };

    const SECOND: Self = Self {
        low: false,
        high: true,
    };
}

/// Vertical inset from a rounded corner at `dist_from_edge` from that edge.
fn corner_inset(radius: f32, dist_from_edge: f32) -> f32 {
    if radius <= 0.0 || dist_from_edge >= radius {
        return 0.0;
    }

    let penetration = radius - dist_from_edge.max(0.0);
    radius - (radius * radius - penetration * penetration).sqrt()
}

/// Sample density per track column in rounded zones (smoother arc edges).
const ROUNDED_SAMPLES_PER_COLUMN: usize = 4;

pub enum Units {
    Normalized,
    Db,
    Octaves,
    Frequency,
    Time,
}

impl Units {
    pub fn format(&self, value: Sample) -> String {
        match self {
            Self::Db => format!("{:+.1} dB", value),
            Self::Normalized => format!("{:.0}%", value * 100.0),
            Self::Octaves => {
                let st = value * 12.0;

                if st == 0.0 {
                    "0 st".to_string()
                } else if st.abs() < 1.0 {
                    format!("{:.0} cents", value * 1_200.0)
                } else {
                    format!("{:.2} st", st)
                }
            }
            Self::Frequency => {
                if value.abs() > 1_000.0 {
                    format!("{:.2} kHz", value / 1_000.0)
                } else {
                    let precision = if value.abs() < 1.0 {
                        2
                    } else if value.abs() < 10.0 {
                        1
                    } else {
                        0
                    };
                    format!("{0:.1$} Hz", value, precision)
                }
            }
            Self::Time => {
                let ms = value * 1_000.0;

                if ms.abs() < 10.0 {
                    format!("{:.1} ms", ms)
                } else if ms.abs() < 1_000.0 {
                    format!("{:.0} ms", ms)
                } else {
                    format!("{:.2} s", value)
                }
            }
        }
    }
}

enum Value<'a> {
    Mono(&'a mut Sample),
    Stereo(&'a mut StereoSample),
}

/// Persistent label visibility state (stored in egui memory, keyed by slider id).
#[derive(Default, Clone)]
struct LabelState {
    visible: bool,
    hover_since: Option<f64>,
}

pub struct Slider<'a> {
    value: Value<'a>,
    range: RangeInclusive<Sample>,
    inverse_to: Option<Sample>,
    over_from: Option<Sample>,
    units: Units,
    skew: Sample,
    default: Option<Sample>,
    length: f32,
    thickness: f32,
    orientation: Orientation,
}

impl<'a> Slider<'a> {
    fn new(value: Value<'a>, range: RangeInclusive<Sample>, inverse_to: Option<Sample>) -> Self {
        assert!(range.end() > range.start());

        if let Some(inverse) = inverse_to {
            assert!(inverse < *range.start());
        }

        Self {
            value,
            range,
            inverse_to,
            over_from: None,
            units: Units::Normalized,
            skew: 1.0,
            default: None,
            length: 160.0,
            thickness: 14.0,
            orientation: Orientation::default(),
        }
    }

    pub fn mono(
        value: &'a mut Sample,
        range: RangeInclusive<Sample>,
        inverse_to: Option<Sample>,
    ) -> Self {
        Self::new(Value::Mono(value), range, inverse_to)
    }

    pub fn stereo(
        value: &'a mut StereoSample,
        range: RangeInclusive<Sample>,
        inverse_to: Option<Sample>,
    ) -> Self {
        Self::new(Value::Stereo(value), range, inverse_to)
    }

    pub fn over(mut self, over_from: Sample) -> Self {
        self.over_from = Some(over_from.clamp(*self.range.start(), *self.range.end()));
        self
    }

    pub fn units(mut self, units: Units) -> Self {
        self.units = units;
        self
    }

    pub fn skew(mut self, skew: Sample) -> Self {
        self.skew = skew;
        self
    }

    pub fn default(mut self, default: Sample) -> Self {
        self.default = Some(default);
        self
    }

    pub fn length(mut self, length: f32) -> Self {
        self.length = length;
        self
    }

    pub fn thickness(mut self, thickness: f32) -> Self {
        self.thickness = thickness;
        self
    }

    pub fn vertical(mut self) -> Self {
        self.orientation = Orientation::Vertical;
        self
    }

    /// Normalize so zero stays zero; scale by the larger of `|from|` and `|to|`.
    fn pre_normalize_scale(&self) -> Sample {
        self.range.end().abs().max(self.range.start().abs())
    }

    fn skew_normalized(&self, normalized: Sample) -> Sample {
        normalized.abs().powf(self.skew.recip()) * normalized.signum()
    }

    fn unskew_normalized(&self, skewed: Sample) -> Sample {
        skewed.abs().powf(self.skew) * skewed.signum()
    }

    /// Value space → skew domain: pre-normalize by range scale, then skew around zero.
    fn to_skew_domain(&self, value: Sample) -> Sample {
        self.skew_normalized(value / self.pre_normalize_scale())
    }

    fn value_to_normalized(&self, value: Sample) -> Sample {
        // 1) normalize in value space (0 → 0, scale by max(|from|, |to|))
        // 2) skew around zero
        let skewed = self.to_skew_domain(value);
        let skewed_from = self.to_skew_domain(*self.range.start());

        // 3) map skewed value onto the slider range / screen
        if value >= *self.range.start() {
            let skewed_to = self.to_skew_domain(*self.range.end());
            ((skewed - skewed_from) / (skewed_to - skewed_from)).clamp(0.0, 1.0)
        } else if let Some(inverse_to) = self.inverse_to {
            let skewed_inverse = self.to_skew_domain(inverse_to);
            ((skewed - skewed_from) / (skewed_from - skewed_inverse)).clamp(-1.0, 0.0)
        } else {
            0.0
        }
    }

    fn normalized_to_value(&self, normalized: Sample) -> Sample {
        let min = if self.inverse_to.is_some() { -1.0 } else { 0.0 };
        let normalized = normalized.clamp(min, 1.0);
        let skewed_from = self.to_skew_domain(*self.range.start());

        let skewed = if normalized >= 0.0 {
            normalized * (self.to_skew_domain(*self.range.end()) - skewed_from) + skewed_from
        } else if let Some(inverse_to) = self.inverse_to {
            normalized * (skewed_from - self.to_skew_domain(inverse_to)) + skewed_from
        } else {
            return 0.0;
        };

        // inverse of skew, then scale back
        self.unskew_normalized(skewed) * self.pre_normalize_scale()
    }

    fn normalized_value(&self) -> StereoSample {
        match &self.value {
            Value::Mono(value) => StereoSample::splat(self.value_to_normalized(**value)),
            Value::Stereo(value) => value.map(|sample| self.value_to_normalized(sample)),
        }
    }

    fn set_value(&mut self, response: &mut Response, new_value: StereoSample) {
        match &mut self.value {
            Value::Mono(value) => **value = new_value.left(),
            Value::Stereo(value) => **value = new_value,
        }
        response.mark_changed();
    }

    fn update_normalized_value(&mut self, response: &mut Response, normalized: StereoSample) {
        let new_value = normalized.map(|sample| self.normalized_to_value(sample));
        self.set_value(response, new_value);
    }

    fn is_mono(&self) -> bool {
        matches!(self.value, Value::Mono(_))
    }

    fn is_stereo(&self) -> bool {
        matches!(self.value, Value::Stereo(_))
    }

    /// Inset at `along_screen` from the rounded corners at the track's along-axis ends.
    fn corner_inset_at(&self, rect: Rect, along_screen: f32) -> f32 {
        let o = self.orientation;
        let d_start = o.dist_from_start(rect, along_screen);
        let d_end = o.dist_from_end(rect, along_screen);
        corner_inset(CORNER_RADIUS, d_start).max(corner_inset(CORNER_RADIUS, d_end))
    }

    /// Across-axis low (top/left) bound of the rounded track silhouette at `along_screen`.
    fn edge_low(&self, rect: Rect, rounded: RoundedEdges, along_screen: f32) -> f32 {
        let inset = self.corner_inset_at(rect, along_screen);
        self.orientation.across_low(rect) + if rounded.low { inset } else { 0.0 }
    }

    /// Across-axis high (bottom/right) bound of the rounded track silhouette at `along_screen`.
    fn edge_high(&self, rect: Rect, rounded: RoundedEdges, along_screen: f32) -> f32 {
        let inset = self.corner_inset_at(rect, along_screen);
        self.orientation.across_high(rect) - if rounded.high { inset } else { 0.0 }
    }

    /// Sample track positions along `[p0, p1]` (in `[0, span]`) matching rounded-track density.
    ///
    /// Every bar shape has radius `CORNER_RADIUS` on both ends of the track, so the
    /// rounded zones are the same regardless of which across-axis corners are rounded.
    fn sample_track_positions(&self, rect: Rect, p0: f32, p1: f32) -> Vec<f32> {
        let o = self.orientation;
        let span = o.along_span(rect);
        let radius = CORNER_RADIUS.min(span * 0.5);

        let mut ts = Vec::new();
        let mut push_t = |t: f32| {
            if ts.last().is_none_or(|&last: &f32| (last - t).abs() > 1e-4) {
                ts.push(t);
            }
        };

        let mut append_range = |range_lo: f32, range_hi: f32, rounded: bool| {
            let lo = range_lo.max(p0);
            let hi = range_hi.min(p1);
            if hi <= lo {
                return;
            }
            let segments = if rounded {
                ((hi - lo) * ROUNDED_SAMPLES_PER_COLUMN as f32).ceil() as usize
            } else {
                1
            };
            for i in 0..=segments {
                push_t(lo + (hi - lo) * i as f32 / segments as f32);
            }
        };

        append_range(0.0, radius, true);
        append_range(radius, span - radius, false);
        append_range(span - radius, span, true);

        ts
    }

    /// Closed clockwise outline for a track segment.
    ///
    /// `position` and `length` are in track-local pixels in `[0, span]`. Returns `None`
    /// when the rect is degenerate or too few points are produced for a polygon.
    fn segment_geometry(
        &self,
        rect: Rect,
        rounded: RoundedEdges,
        position: f32,
        length: f32,
    ) -> Option<Vec<Pos2>> {
        if !rect.is_positive() {
            return None;
        }

        let o = self.orientation;
        let span = o.along_span(rect);
        let p0 = position.clamp(0.0, span);
        let p1 = (position + length).clamp(0.0, span);
        let ts = self.sample_track_positions(rect, p0, p1);

        let mut points = Vec::with_capacity(ts.len() * 2);

        for &t in &ts {
            let along = o.along_screen(rect, t);
            let low = self.edge_low(rect, rounded, along);
            points.push(o.point(along, low));
        }

        for &t in ts.iter().rev() {
            let along = o.along_screen(rect, t);
            let high = self.edge_high(rect, rounded, along);
            points.push(o.point(along, high));
        }

        (points.len() >= 3).then_some(points)
    }

    fn paint_track_stroke(&self, painter: &Painter, rect: Rect, color: Color32) {
        let span = self.orientation.along_span(rect);
        if let Some(points) = self.segment_geometry(rect, RoundedEdges::ALL, 0.0, span) {
            painter.add(Shape::closed_line(
                points,
                PathStroke::new(1.0, color).inside(),
            ));
        }
    }

    fn bar_body_color(&self, end_norm: Sample) -> Color32 {
        if end_norm < 0.0 {
            Color32::from(INVERSE_LEVEL_COLOR)
        } else if self
            .over_from
            .map(|over_from| self.value_to_normalized(over_from))
            .is_some_and(|over| end_norm > over)
        {
            Color32::from(OVER_COLOR)
        } else {
            Color32::from(LEVEL_COLOR)
        }
    }

    fn bar_tip_color(&self, end_norm: Sample) -> Color32 {
        if end_norm < 0.0 {
            Color32::from(INVERSE_LEVEL_TIP_COLOR)
        } else if self
            .over_from
            .map(|over_from| self.value_to_normalized(over_from))
            .is_some_and(|over| end_norm > over)
        {
            Color32::from(OVER_TIP_COLOR)
        } else {
            Color32::from(LEVEL_TIP_COLOR)
        }
    }

    /// Paints the body fill `[start_norm, end_norm]` (no tip), with `over`-zone repaint,
    /// as a single `Shape::mesh`.
    fn paint_bar_body(
        &self,
        painter: &Painter,
        rect: Rect,
        start_norm: Sample,
        end_norm: Sample,
        rounded: RoundedEdges,
    ) {
        let mut mesh = Mesh::with_texture(TextureId::default());

        self.append_body_fan(&mut mesh, rect, rounded, start_norm, end_norm);

        if !mesh.indices.is_empty() {
            painter.add(Shape::mesh(mesh));
        }
    }

    /// Paints the bright tip marker at the leading edge of `end_norm` as a single `Shape::mesh`.
    fn paint_bar_tip(
        &self,
        painter: &Painter,
        rect: Rect,
        end_norm: Sample,
        rounded: RoundedEdges,
    ) {
        let span = self.orientation.along_span(rect);
        let tip_color = self.bar_tip_color(end_norm);
        let position = if end_norm < 0.0 {
            span - end_norm.abs() * span
        } else {
            (end_norm * span - TIP_WIDTH).max(0.0)
        };

        let mut mesh = Mesh::with_texture(TextureId::default());

        if let Some(points) = self.segment_geometry(rect, rounded, position, TIP_WIDTH) {
            self.append_fan(&mut mesh, &points, tip_color);
        }

        if !mesh.indices.is_empty() {
            painter.add(Shape::mesh(mesh));
        }
    }

    /// Paints the bar filling `[start_norm, end_norm]` (normalized, both in `[-1, 1]`).
    ///
    /// `start_norm == 0.0` reproduces the original full-from-bottom behavior. The `over`
    /// zone repaint is shifted to stay relative to `start_norm`.
    fn paint_bar(
        &self,
        painter: &Painter,
        rect: Rect,
        start_norm: Sample,
        end_norm: Sample,
        rounded: RoundedEdges,
    ) {
        let span = self.orientation.along_span(rect);
        let length = (end_norm - start_norm).abs() * span;

        if length <= 0.0 {
            return;
        }

        self.paint_bar_body(painter, rect, start_norm, end_norm, rounded);
        self.paint_bar_tip(painter, rect, end_norm, rounded);
    }

    /// Appends a convex polygon's outline as a triangle fan to `mesh`, all vertices colored `color`.
    fn append_fan(&self, mesh: &mut Mesh, points: &[Pos2], color: Color32) {
        if points.len() < 3 {
            return;
        }
        let base = mesh.vertices.len() as u32;
        for &p in points {
            mesh.vertices.push(Vertex::untextured(p, color));
        }
        let n = points.len() as u32;
        for i in 1..n - 1 {
            mesh.indices.extend([base, base + i, base + i + 1]);
        }
    }

    /// Appends the body fill `[start_norm, end_norm]` (and its `over`-zone repaint) to `mesh`.
    fn append_body_fan(
        &self,
        mesh: &mut Mesh,
        rect: Rect,
        rounded: RoundedEdges,
        start_norm: Sample,
        end_norm: Sample,
    ) {
        let span = self.orientation.along_span(rect);
        let length = (end_norm - start_norm).abs() * span;
        if length <= 0.0 {
            return;
        }

        let (position, body_color) = if end_norm < 0.0 {
            // Inverse: see `paint_bar_body` for the screen-space mapping.
            (
                span - start_norm.abs().max(end_norm.abs()) * span,
                Color32::from(INVERSE_LEVEL_COLOR),
            )
        } else {
            (start_norm.abs() * span, self.bar_body_color(end_norm))
        };

        if let Some(points) = self.segment_geometry(rect, rounded, position, length) {
            self.append_fan(mesh, &points, body_color);
        }

        if end_norm >= 0.0
            && let Some(over_norm) = self
                .over_from
                .map(|v| self.value_to_normalized(v))
                .filter(|&over| end_norm > over)
        {
            let over_pos = (over_norm * span).max(position);
            if let Some(points) =
                self.segment_geometry(rect, rounded, position, (over_pos - position).min(length))
            {
                self.append_fan(mesh, &points, Color32::from(LEVEL_COLOR));
            }
        }
    }

    /// Paints the combined stereo body (common full-width + excess on the taller channel's half)
    /// as a single mesh. For positive values the taller bar is `max(L,R)`; for inverse (both
    /// negative) the taller bar is `min(L,R)` (more negative reaches further).
    fn paint_combined_body(
        &self,
        painter: &Painter,
        rect: Rect,
        first_rect: Rect,
        second_rect: Rect,
        left: Sample,
        right: Sample,
    ) {
        // `common` is the value whose bar is fully covered by the other (the shorter bar);
        // `excess` is the value whose bar sticks out (the taller bar).
        let (common, excess, excess_on_first) = if (left >= 0.0) == (right >= 0.0) {
            // Same sign: positive -> common = min, excess = max; inverse -> common = max, excess = min.
            if left >= 0.0 {
                (left.min(right), left.max(right), left > right)
            } else {
                (left.max(right), left.min(right), left < right)
            }
        } else {
            // Mixed signs: bars don't overlap, fall back to two independent per-half bodies.
            let mut mesh = Mesh::with_texture(TextureId::default());
            self.append_body_fan(&mut mesh, first_rect, RoundedEdges::FIRST, 0.0, left);
            self.append_body_fan(&mut mesh, second_rect, RoundedEdges::SECOND, 0.0, right);
            painter.add(Shape::mesh(mesh));
            return;
        };

        let (excess_rect, excess_rounded) = if excess_on_first {
            (first_rect, RoundedEdges::FIRST)
        } else {
            (second_rect, RoundedEdges::SECOND)
        };

        let mut mesh = Mesh::with_texture(TextureId::default());
        self.append_body_fan(&mut mesh, rect, RoundedEdges::ALL, 0.0, common);
        self.append_body_fan(&mut mesh, excess_rect, excess_rounded, common, excess);
        painter.add(Shape::mesh(mesh));
    }

    fn paint_bars(&self, painter: &Painter, rect: Rect, norm_value: StereoSample) {
        if self.is_stereo() {
            let (first_rect, second_rect) = match self.orientation {
                Orientation::Horizontal => rect.split_top_bottom_at_fraction(0.5),
                Orientation::Vertical => rect.split_left_right_at_fraction(0.5),
            };

            let left = norm_value.left();
            let right = norm_value.right();

            if left == right {
                // Equal channels: render as a single mono bar (with tip).
                self.paint_bar(painter, rect, 0.0, left, RoundedEdges::ALL);
            } else {
                // Combined body (common + excess) as a single geometry, then separate tips.
                self.paint_combined_body(painter, rect, first_rect, second_rect, left, right);
                self.paint_bar_tip(painter, first_rect, left, RoundedEdges::FIRST);
                self.paint_bar_tip(painter, second_rect, right, RoundedEdges::SECOND);
            }
        } else {
            self.paint_bar(painter, rect, 0.0, norm_value.left(), RoundedEdges::ALL);
        }
    }

    fn format_label(&self) -> String {
        match &self.value {
            Value::Mono(value) => self.units.format(**value),
            Value::Stereo(value) if value.left() != value.right() => format!(
                "L: {}, R: {}",
                self.units.format(value.left()),
                self.units.format(value.right())
            ),
            Value::Stereo(value) => self.units.format(value.left()),
        }
    }

    /// Floating value label shown while dragging, as long as the cursor stays over the slider.
    ///
    /// Horizontal: above the track, left-aligned; falls below when it would clip the screen top.
    /// Vertical: to the right of the track, top-aligned; falls to the left when it would clip the
    /// screen right edge. Painted on the `Order::Foreground` layer so it sits above siblings.
    fn paint_drag_label(&self, ui: &Ui, slider_rect: Rect) {
        let text = self.format_label();
        let text_color = Color32::from(LABEL_TEXT_COLOR);
        let bg_color = Color32::from(BG_COLOR);
        let border_color = Color32::from(BORDER_COLOR);

        let painter = ui.painter();
        let galley = painter.layout_no_wrap(
            text,
            FontId::new(11.0, FontFamily::Name("Bold".into())),
            text_color,
        );

        let padding = vec2(LABEL_PADDING, LABEL_PADDING * 0.5);
        let box_size = galley.size() + padding * 2.0;

        let screen = ui.ctx().content_rect();

        let box_rect = match self.orientation {
            Orientation::Horizontal => {
                let left = slider_rect.left();
                let top_above = slider_rect.top() - box_size.y - LABEL_MARGIN;
                let top = if top_above >= screen.top() {
                    top_above
                } else {
                    slider_rect.bottom() + LABEL_MARGIN
                };
                Rect::from_min_size(Pos2::new(left, top), box_size)
            }
            Orientation::Vertical => {
                let top = slider_rect.top();
                let left_right = slider_rect.right() + LABEL_MARGIN;
                let left = if left_right + box_size.x <= screen.right() {
                    left_right
                } else {
                    slider_rect.left() - box_size.x - LABEL_MARGIN
                };
                Rect::from_min_size(Pos2::new(left, top), box_size)
            }
        };

        let fg_painter = painter.clone().with_layer_id(LayerId::new(
            Order::Foreground,
            Id::new("slider-drag-label"),
        ));
        fg_painter.rect(
            box_rect,
            CORNER_RADIUS,
            bg_color,
            Stroke::new(1.0, border_color),
            StrokeKind::Inside,
        );
        fg_painter.galley(box_rect.min + padding, galley, text_color);
    }

    /// Whether the floating value label should be shown right now.
    ///
    /// The label appears on click (any button), stays through the drag, and remains until the
    /// cursor leaves the slider after the drag ends. It also appears after the cursor hovers the
    /// slider for `HOVER_DELAY` without clicking.
    fn label_visible(&self, ui: &Ui, response: &Response) -> bool {
        let now = ui.input(|i| i.time);
        let button_down = response.is_pointer_button_down_on();
        let contains = response.contains_pointer();
        let state_id = response.id.with("label-state");

        ui.memory_mut(|mem| {
            let state = mem.data.get_temp_mut_or_default::<LabelState>(state_id);

            if button_down {
                state.visible = true;
                state.hover_since = None;
            } else if !contains {
                state.visible = false;
                state.hover_since = None;
            } else {
                let start = state.hover_since.get_or_insert(now);
                if now - *start >= HOVER_DELAY {
                    state.visible = true;
                }
            }
            state.visible
        })
    }

    fn add_contents(&mut self, ui: &mut Ui) -> Response {
        let size = match self.orientation {
            Orientation::Horizontal => vec2(self.length, self.thickness),
            Orientation::Vertical => vec2(self.thickness, self.length),
        };
        let mut response = ui.allocate_response(size, Sense::click_and_drag());
        let normalized_value = self.normalized_value();

        if response.dragged() {
            let mut normalized_delta = match self.orientation {
                Orientation::Horizontal => response.drag_delta().x / response.rect.width(),
                // Dragging up should increase the value; screen y grows downward.
                Orientation::Vertical => -response.drag_delta().y / response.rect.height(),
            };

            if ui.input(|state| state.modifiers.shift) {
                normalized_delta *= 0.01;
            }

            if response.dragged_by(PointerButton::Primary)
                || (self.is_mono() && response.dragged_by(PointerButton::Secondary))
            {
                self.update_normalized_value(
                    &mut response,
                    normalized_value + StereoSample::splat(normalized_delta),
                );
            } else if self.is_stereo() && response.dragged_by(PointerButton::Secondary) {
                let is_right = if response.drag_started()
                    && let Some(pos) = response.interact_pointer_pos()
                {
                    let is_right = match self.orientation {
                        Orientation::Horizontal => pos.y >= response.rect.center().y,
                        Orientation::Vertical => pos.x >= response.rect.center().x,
                    };

                    ui.memory_mut(|mem| mem.data.insert_temp(response.id, is_right));
                    is_right
                } else {
                    ui.memory(|mem| mem.data.get_temp(response.id).unwrap_or(false))
                };

                let delta = if is_right {
                    StereoSample::new(0.0, normalized_delta)
                } else {
                    StereoSample::new(normalized_delta, 0.0)
                };
                self.update_normalized_value(&mut response, normalized_value + delta);
            }
        } else if response.double_clicked_by(PointerButton::Primary)
            && let Some(default) = self.default
        {
            self.set_value(&mut response, StereoSample::splat(default));
        }

        if ui.is_rect_visible(response.rect) {
            let painter = ui.painter();

            painter.rect_filled(response.rect, CORNER_RADIUS, Color32::from(BG_COLOR));

            let clip_shrink = match self.orientation {
                Orientation::Horizontal => vec2(1.0, 0.0),
                Orientation::Vertical => vec2(0.0, 1.0),
            };
            self.paint_bars(
                &painter.with_clip_rect(response.rect.shrink2(clip_shrink)),
                response.rect,
                normalized_value,
            );

            self.paint_track_stroke(painter, response.rect, Color32::from(BORDER_COLOR));
        }

        // Label appears on click (any button), stays through the drag, and remains
        // until the cursor leaves the slider after the drag ends. It also appears
        // after the cursor hovers the slider for `HOVER_DELAY` without clicking.
        if self.label_visible(ui, &response) {
            self.paint_drag_label(ui, response.rect);
        }

        response
    }
}

impl Widget for Slider<'_> {
    fn ui(mut self, ui: &mut Ui) -> Response {
        self.add_contents(ui)
    }
}
