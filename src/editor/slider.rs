use std::ops::RangeInclusive;

use egui::{
    Align2, Area, Color32, FontFamily, FontId, Id, Key, LayerId, Margin, Mesh, Order, Painter,
    PointerButton, Pos2, Rect, Response, Sense, Shape, Stroke, StrokeKind, TextEdit, TextureId, Ui,
    Widget,
    ecolor::Hsva,
    emath::GuiRounding,
    epaint::{PathStroke, Vertex},
    text::CCursorRange,
    vec2,
};

use crate::synth_engine::{Sample, StereoSample};

use super::{units::Units, utils::hsva};

const BG_COLOR: Hsva = hsva(0.115, 0.05, 0.01, 1.0);
const BORDER_COLOR: Hsva = hsva(0.115, 0.05, 0.2, 1.0);

const LEVEL_COLOR: Hsva = hsva(0.1, 0.8, 0.15, 1.0);
const LEVEL_TIP_COLOR: Hsva = hsva(0.1, 0.8, 0.4, 1.0);

const INVERSE_LEVEL_COLOR: Hsva = hsva(0.06, 0.8, 0.15, 1.0);
const INVERSE_LEVEL_TIP_COLOR: Hsva = hsva(0.06, 0.8, 0.5, 1.0);

const OVER_COLOR: Hsva = hsva(0.03, 0.8, 0.15, 1.0);
const OVER_TIP_COLOR: Hsva = hsva(0.03, 0.8, 0.5, 1.0);

const CORNER_RADIUS: f32 = 2.0;

const BORDER_WIDTH: f32 = 1.0;
const TIP_WIDTH: f32 = 2.0;

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

    /// Split `rect` into the two stereo channel halves (first = left, second = right).
    fn split_channels(self, rect: Rect) -> (Rect, Rect) {
        match self {
            Self::Horizontal => rect.split_top_bottom_at_fraction(0.5),
            Self::Vertical => rect.split_left_right_at_fraction(0.5),
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

/// Sample density per track column in rounded zones (smoother arc edges).
const ROUNDED_SAMPLES_PER_COLUMN: usize = 4;

enum Value<'a> {
    Mono(&'a mut Sample),
    Stereo(&'a mut StereoSample),
}

#[derive(Default, Clone)]
struct LabelState {
    visible: bool,
    hover_since: Option<f64>,
}

#[derive(Clone, Default)]
struct EnterState {
    text: String,
    request_focus: bool,
}

pub struct Slider<'a> {
    value: Value<'a>,
    modulated_value: Option<StereoSample>,
    range: RangeInclusive<Sample>,
    inverse_to: Option<Sample>,
    over_from: Option<Sample>,
    units: Units,
    skew: Sample,
    default: Option<Sample>,
    length: f32,
    thickness: f32,
    orientation: Orientation,
    show_label: bool,
}

impl<'a> Slider<'a> {
    fn new(value: Value<'a>, range: RangeInclusive<Sample>, inverse_to: Option<Sample>) -> Self {
        assert!(range.end() > range.start());

        if let Some(inverse) = inverse_to {
            assert!(inverse < *range.start());
        }

        Self {
            value,
            modulated_value: None,
            range,
            inverse_to,
            over_from: None,
            units: Units::Normalized,
            skew: 1.0,
            default: None,
            length: 160.0,
            thickness: 14.0,
            orientation: Orientation::default(),
            show_label: false,
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

    pub fn modulated(mut self, value: StereoSample) -> Self {
        self.modulated_value = Some(value);
        self
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

    pub fn show_label(mut self) -> Self {
        self.show_label = true;
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

    fn is_stereo(&self) -> bool {
        matches!(self.value, Value::Stereo(_))
    }

    fn value_min(&self) -> Sample {
        *self.range.start()
    }

    fn stereo_value(&self) -> StereoSample {
        match &self.value {
            Value::Mono(value) => StereoSample::splat(**value),
            Value::Stereo(value) => **value,
        }
    }

    fn context_menu(&mut self, ui: &Ui, response: &mut Response) {
        let min = self.value_min();
        let current = self.stereo_value();
        let mut new_value = None;
        let mut enter = false;

        response.context_menu(|ui| {
            if ui.button("Enter").clicked() {
                enter = true;
                ui.close();
            }

            if self.inverse_to.is_some() && ui.button("Min").clicked() {
                new_value = Some(StereoSample::splat(min));
                ui.close();
            }

            if let Some(default) = self.default
                && ui.button("Default").clicked()
            {
                new_value = Some(StereoSample::splat(default));
                ui.close();
            }

            if self.is_stereo() {
                ui.separator();

                if ui.button("L -> R").clicked() {
                    new_value = Some(StereoSample::splat(current.left()));
                    ui.close();
                }

                if ui.button("R -> L").clicked() {
                    new_value = Some(StereoSample::splat(current.right()));
                    ui.close();
                }
            }
        });

        if enter {
            ui.data_mut(|d| {
                d.insert_temp(
                    response.id.with("enter"),
                    EnterState {
                        text: self.format_edit_text(),
                        request_focus: true,
                    },
                );
            });
        }

        if let Some(new_value) = new_value {
            self.set_value(response, new_value);
        }
    }

    fn parse_entered(&self, text: &str) -> Option<StereoSample> {
        let parsed = self.units.parse(text, self.is_stereo())?;
        let min = self.inverse_to.unwrap_or(*self.range.start());
        let max = *self.range.end();
        Some(parsed.clamp(min, max))
    }

    fn enter_anchor(&self, ui: &Ui, slider_rect: Rect) -> (Align2, Pos2) {
        let screen = ui.ctx().content_rect();

        match self.orientation {
            Orientation::Horizontal => {
                let above = slider_rect.top() - LABEL_MARGIN;
                if above >= screen.top() + 20.0 {
                    (Align2::LEFT_BOTTOM, Pos2::new(slider_rect.left(), above))
                } else {
                    (
                        Align2::LEFT_TOP,
                        Pos2::new(slider_rect.left(), slider_rect.bottom() + LABEL_MARGIN),
                    )
                }
            }
            Orientation::Vertical => {
                let right = slider_rect.right() + LABEL_MARGIN;
                if right + 96.0 <= screen.right() {
                    (Align2::LEFT_TOP, Pos2::new(right, slider_rect.top()))
                } else {
                    (
                        Align2::RIGHT_TOP,
                        Pos2::new(slider_rect.left() - LABEL_MARGIN, slider_rect.top()),
                    )
                }
            }
        }
    }

    fn show_enter_input(&mut self, ui: &mut Ui, response: &mut Response) -> bool {
        let state_id = response.id.with("enter");
        let Some(mut state) = ui.data_mut(|d| d.remove_temp::<EnterState>(state_id)) else {
            return false;
        };

        let (pivot, pos) = self.enter_anchor(ui, response.rect);
        let font = FontId::new(11.0, FontFamily::Name("Bold".into()));
        let text_id = response.id.with("enter-text");
        let text_color = Color32::from(LABEL_TEXT_COLOR);
        let bg_color = Color32::from(BG_COLOR);
        let border_color = Color32::from(BORDER_COLOR);

        let mut output = Area::new(response.id.with("enter-area"))
            .order(Order::Foreground)
            .fixed_pos(pos)
            .pivot(pivot)
            .show(ui.ctx(), |ui| {
                ui.visuals_mut().widgets.inactive.bg_stroke = Stroke::new(1.0, border_color);
                ui.visuals_mut().widgets.hovered.bg_stroke = Stroke::new(1.0, border_color);
                ui.visuals_mut().widgets.active.bg_stroke = Stroke::new(1.0, border_color);

                TextEdit::singleline(&mut state.text)
                    .id(text_id)
                    .desired_width(160.0)
                    .font(font)
                    .text_color(text_color)
                    .background_color(bg_color)
                    .margin(Margin::symmetric(
                        LABEL_PADDING as i8,
                        (LABEL_PADDING * 0.5) as i8,
                    ))
                    .show(ui)
            })
            .inner;

        if state.request_focus {
            output.response.request_focus();
            output
                .state
                .cursor
                .set_char_range(Some(CCursorRange::one(output.galley.end())));
            output.state.store(ui.ctx(), output.response.id);
            state.request_focus = false;
        }

        let lost_focus = output.response.lost_focus();
        let enter = ui.input(|i| i.key_pressed(Key::Enter));
        let escape = ui.input(|i| i.key_pressed(Key::Escape));

        if lost_focus && escape {
            return true;
        }

        if lost_focus {
            if let Some(value) = self.parse_entered(&state.text) {
                self.set_value(response, value);
                return true;
            }

            if enter {
                output.response.request_focus();
            } else {
                return true;
            }
        }

        ui.data_mut(|d| d.insert_temp(state_id, state));
        true
    }

    /// Across-axis inset from a rounded corner at `dist_from_edge` from that edge.
    fn corner_inset(radius: f32, dist_from_edge: f32) -> f32 {
        if radius <= 0.0 || dist_from_edge >= radius {
            return 0.0;
        }

        let penetration = radius - dist_from_edge.max(0.0);
        radius - (radius * radius - penetration * penetration).sqrt()
    }

    /// Inset at `along_screen` from the rounded corners at the track's along-axis ends.
    fn corner_inset_at(&self, rect: Rect, along_screen: f32) -> f32 {
        let o = self.orientation;
        let d_start = o.dist_from_start(rect, along_screen);
        let d_end = o.dist_from_end(rect, along_screen);
        Self::corner_inset(CORNER_RADIUS, d_start).max(Self::corner_inset(CORNER_RADIUS, d_end))
    }

    /// Sample track positions along `[p0, p1]` (in `[0, span]`) matching rounded-track density.
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
    fn segment_outline(
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

        if p1 <= p0 {
            return None;
        }

        let ts = self.sample_track_positions(rect, p0, p1);
        let mut points = Vec::with_capacity(ts.len() * 2 + 4);

        let low = o.across_low(rect);

        if rounded.low {
            for &t in &ts {
                let along = o.along_screen(rect, t);
                points.push(o.point(along, low + self.corner_inset_at(rect, along)));
            }
        } else {
            points.push(o.point(o.along_screen(rect, p0), low));
            points.push(o.point(o.along_screen(rect, p1), low));
        }

        let high = o.across_high(rect);

        if rounded.high {
            for &t in ts.iter().rev() {
                let along = o.along_screen(rect, t);
                points.push(o.point(along, high - self.corner_inset_at(rect, along)));
            }
        } else {
            points.push(o.point(o.along_screen(rect, p1), high));
            points.push(o.point(o.along_screen(rect, p0), high));
        }

        (points.len() >= 3).then_some(points)
    }

    fn paint_track_stroke(&self, painter: &Painter, rect: Rect, color: Color32) {
        let span = self.orientation.along_span(rect);
        if let Some(points) = self.segment_outline(rect, RoundedEdges::ALL, 0.0, span) {
            painter.add(Shape::closed_line(
                points,
                PathStroke::new(BORDER_WIDTH, color).middle(),
            ));
        }
    }

    fn bar_color(&self, norm_value: Sample) -> Color32 {
        if norm_value < 0.0 {
            Color32::from(INVERSE_LEVEL_COLOR)
        } else if self
            .over_from
            .map(|over_from| self.value_to_normalized(over_from))
            .is_some_and(|over| norm_value > over)
        {
            Color32::from(OVER_COLOR)
        } else {
            Color32::from(LEVEL_COLOR)
        }
    }

    fn tip_color(&self, norm_value: Sample) -> Color32 {
        if norm_value < 0.0 {
            Color32::from(INVERSE_LEVEL_TIP_COLOR)
        } else if self
            .over_from
            .map(|over_from| self.value_to_normalized(over_from))
            .is_some_and(|over| norm_value > over)
        {
            Color32::from(OVER_TIP_COLOR)
        } else {
            Color32::from(LEVEL_TIP_COLOR)
        }
    }

    fn paint_tip(&self, painter: &Painter, rect: Rect, norm_value: Sample, rounded: RoundedEdges) {
        let color = self.tip_color(norm_value);
        let span = self.orientation.along_span(rect);
        let abs_position = norm_value.abs() * span;
        let visible_tip_width = if (0.0..TIP_WIDTH).contains(&abs_position) {
            abs_position
        } else {
            TIP_WIDTH
        };

        let (position, length) = if norm_value < 0.0 {
            (span - abs_position - visible_tip_width, TIP_WIDTH)
        } else {
            (abs_position, visible_tip_width)
        };

        let mut mesh = Mesh::with_texture(TextureId::default());
        self.append_segment(&mut mesh, rect, rounded, position, length, color);
        painter.add(Shape::mesh(mesh));
    }

    fn paint_mono_bar(&self, painter: &Painter, rect: Rect, end_norm: Sample) {
        let mut mesh = Mesh::with_texture(TextureId::default());

        self.append_bar_part(&mut mesh, rect, RoundedEdges::ALL, 0.0, end_norm);
        painter.add(Shape::mesh(mesh));
    }

    /// Appends a filled track segment as a triangle fan to `mesh`.
    fn append_segment(
        &self,
        mesh: &mut Mesh,
        rect: Rect,
        rounded: RoundedEdges,
        position: f32,
        length: f32,
        color: Color32,
    ) {
        let Some(points) = self.segment_outline(rect, rounded, position, length) else {
            return;
        };

        let base = mesh.vertices.len() as u32;
        for &p in &points {
            mesh.vertices.push(Vertex::untextured(p, color));
        }
        let n = points.len() as u32;
        for i in 1..n - 1 {
            mesh.indices.extend([base, base + i, base + i + 1]);
        }
    }

    /// Appends the body fill `[start_norm, end_norm]` (and its `over`-zone repaint) to `mesh`.
    fn append_bar_part(
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
            (
                span - start_norm.abs().max(end_norm.abs()) * span,
                Color32::from(INVERSE_LEVEL_COLOR),
            )
        } else {
            (start_norm.abs() * span, self.bar_color(end_norm))
        };

        self.append_segment(mesh, rect, rounded, position, length, body_color);

        if end_norm >= 0.0
            && let Some(over_norm) = self
                .over_from
                .map(|v| self.value_to_normalized(v))
                .filter(|&over| end_norm > over)
        {
            let over_pos = (over_norm * span).max(position);
            self.append_segment(
                mesh,
                rect,
                rounded,
                position,
                (over_pos - position).min(length),
                Color32::from(LEVEL_COLOR),
            );
        }
    }

    fn paint_stereo_bars(&self, painter: &Painter, rect: Rect, norm_value: StereoSample) {
        let [left, right] = *norm_value.channels();
        let (first_rect, second_rect) = self.orientation.split_channels(rect);

        if (left >= 0.0) == (right >= 0.0) {
            let (common, excess, excess_on_left) = if left >= 0.0 {
                (left.min(right), left.max(right), left > right)
            } else {
                (left.max(right), left.min(right), left < right)
            };

            let (excess_rect, excess_rounded) = if excess_on_left {
                (first_rect, RoundedEdges::FIRST)
            } else {
                (second_rect, RoundedEdges::SECOND)
            };

            let mut mesh = Mesh::with_texture(TextureId::default());

            self.append_bar_part(&mut mesh, rect, RoundedEdges::ALL, 0.0, common);
            self.append_bar_part(&mut mesh, excess_rect, excess_rounded, common, excess);
            painter.add(Shape::mesh(mesh));
        } else {
            // Mixed signs: bars don't overlap, fall back to two independent per-half bars.
            let mut mesh = Mesh::with_texture(TextureId::default());

            self.append_bar_part(&mut mesh, first_rect, RoundedEdges::FIRST, 0.0, left);
            self.append_bar_part(&mut mesh, second_rect, RoundedEdges::SECOND, 0.0, right);
            painter.add(Shape::mesh(mesh));
        };
    }

    fn paint_bars(&self, painter: &Painter, rect: Rect, norm_value: StereoSample) {
        if self.is_stereo() {
            self.paint_stereo_bars(painter, rect, norm_value);

            let [left, right] = if let Some(modulated) = self.modulated_value {
                *modulated.map(|v| self.value_to_normalized(v)).channels()
            } else {
                *norm_value.channels()
            };

            if (left - right).abs() < 1e-6 {
                self.paint_tip(painter, rect, left, RoundedEdges::ALL);
            } else {
                let (first_rect, second_rect) = self.orientation.split_channels(rect);

                self.paint_tip(painter, first_rect, left, RoundedEdges::FIRST);
                self.paint_tip(painter, second_rect, right, RoundedEdges::SECOND);
            }
        } else {
            let norm_value = norm_value.left();

            self.paint_mono_bar(painter, rect, norm_value);
            self.paint_tip(painter, rect, norm_value, RoundedEdges::ALL);
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

    fn format_edit_text(&self) -> String {
        match &self.value {
            Value::Mono(value) => self.units.format_input(**value),
            Value::Stereo(value) if value.left() != value.right() => format!(
                "{}, {}",
                self.units.format_input(value.left()),
                self.units.format_input(value.right())
            ),
            Value::Stereo(value) => self.units.format_input(value.left()),
        }
    }

    fn paint_label(&self, ui: &Ui, slider_rect: Rect) {
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

    fn label_visible(&self, ui: &Ui, response: &Response) -> bool {
        let now = ui.input(|i| i.time);
        let button_down_on = response.is_pointer_button_down_on();
        let button_down = ui.input(|i| i.pointer.primary_down() || i.pointer.secondary_down());
        let contains = response.contains_pointer();
        let menu_open = response.context_menu_opened();
        let state_id = response.id.with("label-state");

        ui.data_mut(|d| {
            let state = d.get_temp_mut_or_default::<LabelState>(state_id);

            if menu_open {
                state.visible = false;
                state.hover_since = None;
            } else if button_down_on {
                state.visible = true;
                state.hover_since = None;
            } else if !contains || button_down {
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
                Orientation::Vertical => -response.drag_delta().y / response.rect.height(),
            };

            if ui.input(|state| state.modifiers.shift) {
                normalized_delta *= 0.01;
            }

            if response.dragged_by(PointerButton::Primary)
                || (!self.is_stereo() && response.dragged_by(PointerButton::Secondary))
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

                    ui.data_mut(|d| d.insert_temp(response.id, is_right));
                    is_right
                } else {
                    ui.data(|d| d.get_temp(response.id).unwrap_or(false))
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
            let rect = response.rect.round_to_pixel_center(ui.pixels_per_point());

            painter.rect_filled(rect, CORNER_RADIUS, Color32::from(BG_COLOR));
            self.paint_bars(painter, rect, normalized_value);
            self.paint_track_stroke(painter, rect, Color32::from(BORDER_COLOR));
        }

        self.context_menu(ui, &mut response);

        if !self.show_enter_input(ui, &mut response) && self.label_visible(ui, &response) {
            self.paint_label(ui, response.rect);
        }

        response
    }
}

impl Widget for Slider<'_> {
    fn ui(mut self, ui: &mut Ui) -> Response {
        if self.show_label && self.orientation == Orientation::Horizontal {
            ui.horizontal(|ui| {
                let response = self.add_contents(ui);
                ui.label(self.format_label());
                response
            })
            .inner
        } else {
            self.add_contents(ui)
        }
    }
}
