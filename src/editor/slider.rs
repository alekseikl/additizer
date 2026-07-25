use std::ops::RangeInclusive;

use egui::{
    Color32, Painter, PointerButton, Pos2, Rect, Response, Sense, Shape, Ui, Widget, ecolor::Hsva,
    epaint::PathStroke, vec2,
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

/// Which horizontal edges of a bar have rounded corners.
#[derive(Clone, Copy)]
struct RoundedEdges {
    top: bool,
    bottom: bool,
}

impl RoundedEdges {
    const ALL: Self = Self {
        top: true,
        bottom: true,
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

/// Top/bottom y of the rounded track silhouette at absolute `x`.
fn edge_ys(rect: Rect, rounded: RoundedEdges, x: f32) -> (f32, f32) {
    let d_left = x - rect.left();
    let d_right = rect.right() - x;
    let inset = corner_inset(CORNER_RADIUS, d_left).max(corner_inset(CORNER_RADIUS, d_right));

    let top = rect.top() + if rounded.top { inset } else { 0.0 };
    let bottom = rect.bottom() - if rounded.bottom { inset } else { 0.0 };

    (top, bottom)
}

/// Sample density per pixel column in rounded zones (smoother arc edges).
const ROUNDED_SAMPLES_PER_COLUMN: usize = 4;

/// Sample x positions along `[x0, x1]` matching rounded-track density.
///
/// Every bar shape has radius `CORNER_RADIUS` on both the left and right side,
/// so the rounded x-zones are the same regardless of which corners are rounded.
fn sample_xs(rect: Rect, x0: f32, x1: f32) -> Vec<f32> {
    let radius = CORNER_RADIUS.min(rect.width() * 0.5);

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
        let segments = if rounded {
            ((right - left) * ROUNDED_SAMPLES_PER_COLUMN as f32).ceil() as usize
        } else {
            1
        };
        for i in 0..=segments {
            push_x(left + (right - left) * i as f32 / segments as f32);
        }
    };

    append_range(rect.left(), rect.left() + radius, true);
    append_range(rect.left() + radius, rect.right() - radius, false);
    append_range(rect.right() - radius, rect.right(), true);

    xs
}

/// Closed outline (clockwise: top L→R, bottom R→L) for a horizontal track segment.
///
/// `position` and `length` are in track-local pixels in `[0, rect.width()]`.
fn segment_geometry(rect: Rect, rounded: RoundedEdges, position: f32, length: f32) -> Vec<Pos2> {
    if !rect.is_positive() {
        return Vec::new();
    }

    let width = rect.width();
    let x0 = rect.left() + position.clamp(0.0, width);
    let x1 = rect.left() + (position + length).clamp(0.0, width);
    let xs = sample_xs(rect, x0, x1);

    if xs.len() < 2 {
        return Vec::new();
    }

    let mut points = Vec::with_capacity(xs.len() * 2);

    for &x in &xs {
        let (top, _) = edge_ys(rect, rounded, x);
        points.push(Pos2::new(x, top));
    }

    for &x in xs.iter().rev() {
        let (_, bottom) = edge_ys(rect, rounded, x);
        points.push(Pos2::new(x, bottom));
    }
    points
}

fn paint_segment(
    painter: &Painter,
    rect: Rect,
    rounded: RoundedEdges,
    position: f32,
    length: f32,
    color: Color32,
) {
    let points = segment_geometry(rect, rounded, position, length);

    if points.len() < 3 {
        return;
    }

    painter.add(Shape::convex_polygon(points, color, PathStroke::NONE));
}

fn paint_track_stroke(painter: &Painter, rect: Rect, color: Color32) {
    let points = segment_geometry(rect, RoundedEdges::ALL, 0.0, rect.width());
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

    fn paint_bar(&self, painter: &Painter, rect: Rect, norm_value: Sample, rounded: RoundedEdges) {
        let width = rect.width();
        let length = norm_value.abs() * width;

        if length <= 0.0 {
            return;
        }

        if norm_value < 0.0 {
            let position = width - length;
            let color = Color32::from(INVERSE_LEVEL_COLOR);
            let tip_color = Color32::from(INVERSE_LEVEL_TIP_COLOR);

            paint_segment(painter, rect, rounded, position, length, color);
            paint_segment(painter, rect, rounded, position, TIP_WIDTH, tip_color);
            return;
        }

        // The `over` zone is painted by filling the whole bar in the over color,
        // then repainting `[0, over_norm]` in the normal color on top.
        let over_norm = self
            .over_from
            .map(|over_from| self.value_to_normalized(over_from))
            .filter(|&over| norm_value > over);

        let (body_color, tip_color) = if over_norm.is_some() {
            (OVER_COLOR, OVER_TIP_COLOR)
        } else {
            (LEVEL_COLOR, LEVEL_TIP_COLOR)
        };

        paint_segment(painter, rect, rounded, 0.0, length, body_color.into());

        if let Some(over_norm) = over_norm {
            paint_segment(
                painter,
                rect,
                rounded,
                0.0,
                over_norm * width,
                Color32::from(LEVEL_COLOR),
            );
        }

        paint_segment(
            painter,
            rect,
            rounded,
            length - TIP_WIDTH,
            TIP_WIDTH,
            tip_color.into(),
        );
    }

    fn paint_bars(&self, painter: &Painter, rect: Rect, normalized_value: StereoSample) {
        if self.is_stereo() {
            let (top, bottom) = rect.split_top_bottom_at_fraction(0.5);
            let top_rounded = RoundedEdges {
                top: true,
                bottom: false,
            };
            let bottom_rounded = RoundedEdges {
                top: false,
                bottom: true,
            };

            self.paint_bar(painter, top, normalized_value.left(), top_rounded);
            self.paint_bar(painter, bottom, normalized_value.right(), bottom_rounded);
        } else {
            self.paint_bar(painter, rect, normalized_value.left(), RoundedEdges::ALL);
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
                let is_right = if response.drag_started()
                    && let Some(pos) = response.interact_pointer_pos()
                {
                    let is_right = pos.y >= response.rect.center().y;

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

            self.paint_bars(
                &painter.with_clip_rect(response.rect.shrink2(vec2(1.0, 0.0))),
                response.rect,
                normalized_value,
            );

            paint_track_stroke(painter, response.rect, Color32::from(BORDER_COLOR));
        }

        response.on_hover_text_at_pointer(self.format_label())
    }
}

impl Widget for Slider<'_> {
    fn ui(mut self, ui: &mut Ui) -> Response {
        self.add_contents(ui)
    }
}
