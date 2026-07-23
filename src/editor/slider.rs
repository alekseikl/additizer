use std::ops::RangeInclusive;

use egui::{
    Color32, PointerButton, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2, Widget,
    ecolor::Hsva, vec2,
};

use crate::synth_engine::{Sample, StereoSample};

const fn hsva(h: f32, s: f32, v: f32, a: f32) -> Hsva {
    Hsva { h, s, v, a }
}

const BG_COLOR: Hsva = hsva(0.115, 0.1, 0.005, 1.0);
const BORDER_COLOR: Hsva = hsva(0.115, 0.35, 0.3, 1.0);

const LEVEL_COLOR: Hsva = hsva(0.1, 0.8, 0.2, 1.0);
const LEVEL_CAP_COLOR: Hsva = hsva(0.1, 0.8, 0.5, 1.0);

const INVERSE_LEVEL_COLOR: Hsva = hsva(0.06, 0.8, 0.2, 1.0);
const INVERSE_LEVEL_CAP_COLOR: Hsva = hsva(0.06, 0.8, 0.5, 1.0);

const OVER_COLOR: Hsva = hsva(0.03, 0.8, 0.2, 1.0);
const OVER_CAP_COLOR: Hsva = hsva(0.03, 0.8, 0.5, 1.0);

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

    fn response_size(&self) -> Vec2 {
        vec2(self.length, self.thickness)
    }

    fn is_right_channel(&self, pos: Pos2, response: &Response) -> bool {
        pos.y >= response.rect.center().y
    }

    fn is_stereo(&self) -> bool {
        matches!(self.value, Value::Stereo(_))
    }

    fn paint_bar(&self, ui: &mut Ui, rect: Rect, norm_value: Sample) {
        let width = rect.width();

        let paint_right_cap = |filled: Rect, cap: Hsva| {
            if filled.width() < 1.0 {
                return;
            }
            let cap_rect =
                Rect::from_min_max(Pos2::new(filled.right() - 1.0, filled.top()), filled.max);
            ui.painter().rect_filled(cap_rect, 0.0, Color32::from(cap));
        };

        let paint_left_cap = |filled: Rect, cap: Hsva| {
            if filled.width() < 1.0 {
                return;
            }
            let cap_rect =
                Rect::from_min_max(filled.min, Pos2::new(filled.left() + 1.0, filled.bottom()));
            ui.painter().rect_filled(cap_rect, 0.0, Color32::from(cap));
        };

        if norm_value < 0.0 {
            let filled = Rect::from_min_max(
                Pos2::new(rect.left() + (1.0 + norm_value) * width, rect.top()),
                rect.max,
            );
            ui.painter()
                .rect_filled(filled, 0.0, Color32::from(INVERSE_LEVEL_COLOR));
            paint_left_cap(filled, INVERSE_LEVEL_CAP_COLOR);
            return;
        }

        let filled = Rect::from_min_max(
            rect.min,
            Pos2::new(rect.left() + norm_value * width, rect.max.y),
        );

        let over_norm = self
            .over_from
            .map(|over_from| self.value_to_normalized(over_from));

        if let Some(over_norm) = over_norm
            && norm_value > over_norm
        {
            ui.painter()
                .rect_filled(filled, 0.0, Color32::from(OVER_COLOR));
            let normal = Rect::from_min_max(
                rect.min,
                Pos2::new(rect.left() + over_norm * width, rect.max.y),
            );
            ui.painter()
                .rect_filled(normal, 0.0, Color32::from(LEVEL_COLOR));
            paint_right_cap(filled, OVER_CAP_COLOR);
        } else {
            ui.painter()
                .rect_filled(filled, 0.0, Color32::from(LEVEL_COLOR));
            paint_right_cap(filled, LEVEL_CAP_COLOR);
        }
    }

    fn paint_bars(&self, ui: &mut Ui, response: &Response, normalized_value: StereoSample) {
        if self.is_stereo() {
            let lr_rect = response.rect.split_top_bottom_at_fraction(0.5);
            self.paint_bar(ui, lr_rect.0, normalized_value.left());
            self.paint_bar(ui, lr_rect.1, normalized_value.right());
        } else {
            self.paint_bar(ui, response.rect, normalized_value.left());
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
        let mut response = ui.allocate_response(self.response_size(), Sense::click_and_drag());
        let normalized_value = self.normalized_value();

        if self.is_stereo()
            && let Some(pos) = response.interact_pointer_pos()
            && response.drag_started_by(PointerButton::Secondary)
        {
            ui.memory_mut(|mem| {
                mem.data
                    .insert_temp(response.id, self.is_right_channel(pos, &response))
            });
        }

        if response.dragged() {
            let mut normalized_delta = response.drag_delta().x / response.rect.width();

            if ui.input(|state| state.modifiers.shift) {
                normalized_delta *= 0.01;
            }

            if response.dragged_by(PointerButton::Primary) {
                self.update_normalized_value(
                    &mut response,
                    normalized_value + StereoSample::splat(normalized_delta),
                );
            } else if self.is_stereo() && response.dragged_by(PointerButton::Secondary) {
                let is_right_channel =
                    ui.memory(|mem| mem.data.get_temp(response.id).unwrap_or(false));

                let delta = if is_right_channel {
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
            ui.painter()
                .rect_filled(response.rect, 0.0, Color32::from(BG_COLOR));
            self.paint_bars(ui, &response, normalized_value);
            ui.painter().rect_stroke(
                response.rect,
                0.0,
                Stroke::new(1.0, Color32::from(BORDER_COLOR)),
                StrokeKind::Inside,
            );
        }

        response.on_hover_text_at_pointer(self.format_label())
    }
}

impl Widget for Slider<'_> {
    fn ui(mut self, ui: &mut Ui) -> Response {
        self.add_contents(ui)
    }
}
