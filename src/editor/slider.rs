use std::ops::RangeInclusive;

use egui::{
    Color32, CornerRadius, Painter, PointerButton, Pos2, Rect, Response, Sense, Shape, Ui, Vec2,
    Widget, ecolor::Hsva, epaint::PathStroke, vec2,
};

use crate::synth_engine::{Sample, StereoSample};

const fn hsva(h: f32, s: f32, v: f32, a: f32) -> Hsva {
    Hsva { h, s, v, a }
}

const BG_COLOR: Hsva = hsva(0.115, 0.1, 0.005, 1.0);
const BORDER_COLOR: Hsva = hsva(0.115, 0.35, 0.3, 1.0);

const LEVEL_COLOR: Hsva = hsva(0.1, 0.8, 0.15, 1.0);
const LEVEL_TIP_COLOR: Hsva = hsva(0.1, 0.8, 0.5, 1.0);

const INVERSE_LEVEL_COLOR: Hsva = hsva(0.06, 0.8, 0.15, 1.0);
const INVERSE_LEVEL_TIP_COLOR: Hsva = hsva(0.06, 0.8, 0.5, 1.0);

const OVER_COLOR: Hsva = hsva(0.03, 0.8, 0.15, 1.0);
const OVER_TIP_COLOR: Hsva = hsva(0.03, 0.8, 0.5, 1.0);

const CORNER_RADIUS: f32 = 4.0;

/// Width of the bright leading tip marker at the fill's edge.
const TIP_WIDTH: f32 = 1.0;

/// Vertical inset from a rounded corner at `dist_from_edge` from that edge.
fn corner_inset(radius: f32, dist_from_edge: f32) -> f32 {
    if radius <= 0.0 || dist_from_edge >= radius {
        return 0.0;
    }

    let penetration = radius - dist_from_edge.max(0.0);
    radius - (radius * radius - penetration * penetration).sqrt()
}

/// Top/bottom y of the rounded track silhouette at absolute `x`.
fn edge_ys(rect: Rect, corners: CornerRadius, x: f32) -> (f32, f32) {
    let d_left = x - rect.left();
    let d_right = rect.right() - x;

    let top_inset =
        corner_inset(corners.nw as f32, d_left).max(corner_inset(corners.ne as f32, d_right));
    let bottom_inset =
        corner_inset(corners.sw as f32, d_left).max(corner_inset(corners.se as f32, d_right));

    (rect.top() + top_inset, rect.bottom() - bottom_inset)
}

/// Sample density per pixel column in rounded zones (smoother arc edges).
const ROUNDED_SAMPLES_PER_COLUMN: usize = 4;

/// Sample x positions along `[x0, x1]` matching rounded-track density.
fn sample_xs(rect: Rect, corners: CornerRadius, x0: f32, x1: f32) -> Vec<f32> {
    if x1 <= x0 {
        return Vec::new();
    }

    let width = rect.width();
    let left_r = (corners.nw as f32).max(corners.sw as f32).min(width * 0.5);
    let right_r = (corners.ne as f32).max(corners.se as f32).min(width * 0.5);
    let round_left_end = rect.left() + left_r;
    let round_right_start = rect.right() - right_r;

    let mut xs = Vec::new();
    let mut push_x = |x: f32| {
        if xs.last().is_none_or(|&last: &f32| (last - x).abs() > 1e-4) {
            xs.push(x);
        }
    };

    let mut append_range = |range_left: f32, range_right: f32, rounded: bool| {
        let left = range_left.max(x0);
        let right = range_right.min(x1);
        if right <= left {
            return;
        }
        if !rounded {
            push_x(left);
            push_x(right);
            return;
        }

        let mut x = left;
        while x < right {
            let col_end = (x.floor() + 1.0).min(right);
            let step = (col_end - x) / ROUNDED_SAMPLES_PER_COLUMN as f32;
            let mut sub = x;
            for _ in 0..ROUNDED_SAMPLES_PER_COLUMN {
                push_x(sub);
                sub = (sub + step).min(col_end);
            }
            push_x(col_end);
            x = col_end;
        }
    };

    append_range(rect.left(), round_left_end.min(rect.right()), left_r > 0.0);
    append_range(
        round_left_end.max(rect.left()),
        round_right_start.min(rect.right()),
        false,
    );
    append_range(
        round_right_start.max(rect.left()),
        rect.right(),
        right_r > 0.0,
    );

    xs
}

/// Closed outline (clockwise: top L→R, bottom R→L) for a horizontal track segment.
///
/// `position` and `length` are in track-local pixels in `[0, rect.width()]`.
fn segment_geometry(rect: Rect, corners: CornerRadius, position: f32, length: f32) -> Vec<Pos2> {
    let width = rect.width();

    if width <= 0.0 || length <= 0.0 || !rect.is_positive() {
        return Vec::new();
    }

    let start = position.clamp(0.0, width);
    let end = (position + length).clamp(0.0, width);

    if end <= start {
        return Vec::new();
    }

    let xs = sample_xs(rect, corners, rect.left() + start, rect.left() + end);

    if xs.len() < 2 {
        return Vec::new();
    }

    let mut points = Vec::with_capacity(xs.len() * 2);

    for &x in &xs {
        let (top, _) = edge_ys(rect, corners, x);
        points.push(Pos2::new(x, top));
    }

    for &x in xs.iter().rev() {
        let (_, bottom) = edge_ys(rect, corners, x);
        points.push(Pos2::new(x, bottom));
    }
    points
}

fn paint_segment(
    painter: &Painter,
    rect: Rect,
    corners: CornerRadius,
    position: f32,
    length: f32,
    color: Color32,
) {
    let points = segment_geometry(rect, corners, position, length);

    if points.len() < 3 {
        return;
    }

    painter.add(Shape::convex_polygon(points, color, PathStroke::NONE));
}

fn paint_track_stroke(painter: &Painter, rect: Rect, corners: CornerRadius, color: Color32) {
    let points = segment_geometry(rect, corners, 0.0, rect.width());
    if points.len() < 3 {
        return;
    }

    painter.add(Shape::closed_line(
        points,
        PathStroke::new(1.0, color).inside(),
    ));
}

pub enum Units {
    Normalized,
    Db,
    Percents,
    Octaves,
    Frequency,
    Time,
}

impl Units {
    pub fn format(&self, value: Sample) -> String {
        match self {
            Self::Normalized => format!("{:.2}", value),
            Self::Db => format!("{:+.1} dB", value),
            Self::Percents => format!("{:.0}%", value * 100.0),
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

    /// Normalize so zero stays zero; scale by the larger of `|from|` and `|to|`.
    fn pre_normalize_scale(&self) -> Sample {
        self.range.end().abs().max(self.range.start().abs())
    }

    fn pre_normalize(&self, value: Sample) -> Sample {
        value * self.pre_normalize_scale().recip()
    }

    fn skew_normalized(&self, normalized: Sample) -> Sample {
        normalized.abs().powf(self.skew.recip()) * normalized.signum()
    }

    fn unskew_normalized(&self, skewed: Sample) -> Sample {
        skewed.abs().powf(self.skew) * skewed.signum()
    }

    /// Value space → skew domain: pre-normalize by range scale, then skew around zero.
    fn to_skew_domain(&self, value: Sample) -> Sample {
        self.skew_normalized(self.pre_normalize(value))
    }

    fn value_to_normalized(&self, value: Sample) -> Sample {
        // 1) normalize in value space (0 → 0, scale by max(|from|, |to|))
        // 2) skew around zero
        let skewed = self.to_skew_domain(value);
        let skewed_from = self.to_skew_domain(*self.range.start());

        // 3) map skewed value onto the slider range / screen
        if value >= *self.range.start() {
            let skewed_to = self.to_skew_domain(*self.range.end());
            ((skewed - skewed_from) * (skewed_to - skewed_from).recip()).clamp(0.0, 1.0)
        } else if let Some(inverse_to) = self.inverse_to {
            let skewed_inverse = self.to_skew_domain(inverse_to);
            ((skewed - skewed_from) * (skewed_from - skewed_inverse).recip()).clamp(-1.0, 0.0)
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

    fn update_normalized_value(&mut self, response: &mut Response, normalized: StereoSample) {
        let left = self.normalized_to_value(normalized.left());
        let right = self.normalized_to_value(normalized.right());

        match &mut self.value {
            Value::Mono(value) => **value = left,
            Value::Stereo(value) => **value = StereoSample::new(left, right),
        }
        response.mark_changed();
    }

    fn is_mono(&self) -> bool {
        matches!(self.value, Value::Mono(_))
    }

    fn is_stereo(&self) -> bool {
        matches!(self.value, Value::Stereo(_))
    }

    fn paint_bar(&self, painter: &Painter, rect: Rect, norm_value: Sample, corners: CornerRadius) {
        let width = rect.width();

        if norm_value < 0.0 {
            let length = -norm_value * width;
            let position = width - length;

            if length <= 0.0 {
                return;
            }

            paint_segment(
                painter,
                rect,
                corners,
                position,
                length,
                Color32::from(INVERSE_LEVEL_COLOR),
            );

            paint_segment(
                painter,
                rect,
                corners,
                position,
                TIP_WIDTH,
                Color32::from(INVERSE_LEVEL_TIP_COLOR),
            );
            return;
        }

        let length = norm_value * width;

        if length <= 0.0 {
            return;
        }

        let over_norm = self
            .over_from
            .map(|over_from| self.value_to_normalized(over_from));

        let (body_color, tip_color) = if over_norm.is_some_and(|over| norm_value > over) {
            (OVER_COLOR, OVER_TIP_COLOR)
        } else {
            (LEVEL_COLOR, LEVEL_TIP_COLOR)
        };

        paint_segment(
            painter,
            rect,
            corners,
            0.0,
            length,
            Color32::from(body_color),
        );

        if let Some(over_norm) = over_norm
            && norm_value > over_norm
        {
            let over_len = over_norm * width;

            if over_len > 0.0 {
                paint_segment(
                    painter,
                    rect,
                    corners,
                    0.0,
                    over_len,
                    Color32::from(LEVEL_COLOR),
                );
            }
        }

        paint_segment(
            painter,
            rect,
            corners,
            length - TIP_WIDTH,
            TIP_WIDTH,
            Color32::from(tip_color),
        );
    }

    fn paint_bars(&self, painter: &Painter, rect: Rect, normalized_value: StereoSample) {
        let r = CORNER_RADIUS as u8;

        if self.is_stereo() {
            let lr_rect = rect.split_top_bottom_at_fraction(0.5);

            self.paint_bar(
                painter,
                lr_rect.0,
                normalized_value.left(),
                CornerRadius {
                    nw: r,
                    ne: r,
                    ..CornerRadius::ZERO
                },
            );
            self.paint_bar(
                painter,
                lr_rect.1,
                normalized_value.right(),
                CornerRadius {
                    sw: r,
                    se: r,
                    ..CornerRadius::ZERO
                },
            );
        } else {
            self.paint_bar(
                painter,
                rect,
                normalized_value.left(),
                CornerRadius::same(r),
            );
        }
    }

    fn format_label(&self) -> String {
        match &self.value {
            Value::Mono(value) => self.units.format(**value),
            Value::Stereo(value) if value.left() != value.right() => format!(
                "(L: {}, R: {})",
                self.units.format(value.left()),
                self.units.format(value.right())
            ),
            Value::Stereo(value) => self.units.format(value.left()),
        }
    }

    fn add_contents(&mut self, ui: &mut Ui) -> Response {
        let mut response =
            ui.allocate_response(vec2(self.length, self.thickness), Sense::click_and_drag());
        let normalized_value = self.normalized_value();

        if response.dragged() {
            let mut normalized_delta = response.drag_delta().x / response.rect.width();

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
                let delta = if let Some(pos) = response.interact_pointer_pos()
                    && pos.y >= response.rect.center().y
                {
                    StereoSample::new(0.0, normalized_delta)
                } else {
                    StereoSample::new(normalized_delta, 0.0)
                };
                self.update_normalized_value(&mut response, normalized_value + delta);
            }
        } else if response.double_clicked_by(PointerButton::Primary)
            && let Some(default) = self.default
        {
            match &mut self.value {
                Value::Mono(value) => **value = default,
                Value::Stereo(value) => **value = StereoSample::splat(default),
            }
            response.mark_changed();
        }

        if ui.is_rect_visible(response.rect) {
            let corners = CornerRadius::same(CORNER_RADIUS as u8);
            let painter = ui.painter();

            painter.rect_filled(response.rect, CORNER_RADIUS, Color32::from(BG_COLOR));

            self.paint_bars(
                &painter.with_clip_rect(response.rect.shrink2(vec2(1.0, 0.0))),
                response.rect,
                normalized_value,
            );

            paint_track_stroke(painter, response.rect, corners, Color32::from(BORDER_COLOR));
        }

        response.on_hover_text_at_pointer(self.format_label())
    }
}

impl Widget for Slider<'_> {
    fn ui(mut self, ui: &mut Ui) -> Response {
        self.add_contents(ui)
    }
}
