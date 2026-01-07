use std::ops::RangeInclusive;

use egui_baseview::egui::{Color32, PointerButton, Rect, Response, Sense, Ui, Widget, vec2};

use crate::synth_engine::{Sample, StereoSample};

const BG_COLOR: Color32 = Color32::from_rgb(0, 0, 0);
const LEVEL_COLOR: Color32 = Color32::from_rgb(0x0b, 0x42, 0x67);
const NEGATIVE_LEVEL_COLOR: Color32 = Color32::from_rgb(0x72, 0x72, 0x12);
const SLIDER_HEIGHT: f32 = 16.0;

pub struct StereoSlider<'a> {
    units: Option<&'a str>,
    value: &'a mut StereoSample,
    range: RangeInclusive<Sample>,
    default: Option<Sample>,
    precision: usize,
    skew_factor: Sample,
    display_scale_factor: Sample,
    width: f32,
    color: Color32,
    allow_inverse: bool,
}

impl<'a> StereoSlider<'a> {
    pub fn new(value: &'a mut StereoSample) -> Self {
        Self {
            skew_factor: 1.0,
            display_scale_factor: 1.0,
            width: 200.0,
            color: LEVEL_COLOR,
            units: None,
            default: None,
            precision: 1,
            value,
            range: 0.0..=1.0,
            allow_inverse: false,
        }
    }

    pub fn range(mut self, range: RangeInclusive<Sample>) -> Self {
        self.range = range;
        self
    }

    pub fn units(mut self, units: &'a str) -> Self {
        self.units = Some(units);
        self
    }

    pub fn precision(mut self, precision: usize) -> Self {
        self.precision = precision;
        self
    }

    pub fn default_value(mut self, default: Sample) -> Self {
        self.default = Some(default);
        self
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    #[allow(unused)]
    pub fn color(mut self, color: Color32) -> Self {
        self.color = color;
        self
    }

    pub fn skew(mut self, skew: Sample) -> Self {
        self.skew_factor = skew;
        self
    }

    pub fn display_scale(mut self, scale: Sample) -> Self {
        self.display_scale_factor = scale;
        self
    }

    pub fn allow_inverse(mut self) -> Self {
        self.allow_inverse = true;
        self
    }

    fn normalized_minimum(&self) -> Sample {
        if self.allow_inverse { -1.0 } else { 0.0 }
    }

    fn skew_value(&self, norm_value: StereoSample) -> StereoSample {
        norm_value.abs().powf(self.skew_factor.recip()) * norm_value.signum()
    }

    fn unskew_value(&self, norm_value: StereoSample) -> StereoSample {
        norm_value.abs().powf(self.skew_factor) * norm_value.signum()
    }

    fn normalized_value(&self) -> StereoSample {
        let start = *self.range.start();
        let end = *self.range.end();
        let min_normalized = self.normalized_minimum();
        let clamped = ((*self.value - start) * (end - start).recip()).clamp(min_normalized, 1.0);

        self.skew_value(clamped)
    }

    fn update_normalized_value(&mut self, response: &mut Response, normalized: StereoSample) {
        let start = *self.range.start();
        let end = *self.range.end();
        let min_normalized = self.normalized_minimum();

        *self.value =
            self.unskew_value(normalized).clamp(min_normalized, 1.0) * (end - start) + start;
        response.mark_changed();
    }

    fn format_value(&self, value: Sample) -> String {
        format!("{0:.1$}", value * self.display_scale_factor, self.precision)
    }

    fn add_contents(&mut self, ui: &mut Ui) -> Response {
        let mut response =
            ui.allocate_response(vec2(self.width, SLIDER_HEIGHT), Sense::click_and_drag());
        let normalized_value = self.normalized_value();

        if let Some(pos) = response.interact_pointer_pos()
            && response.drag_started_by(PointerButton::Secondary)
        {
            let is_right_channel = pos.y >= response.rect.center().y;

            ui.memory_mut(|mem| mem.data.insert_temp(response.id, is_right_channel));
        }

        if response.dragged() {
            let mut normalized_delta = response.drag_delta().x / response.rect.width();

            if ui.input(|state| state.modifiers.shift) {
                normalized_delta *= 0.01;
            }

            if response.dragged_by(PointerButton::Primary) {
                self.update_normalized_value(&mut response, normalized_value + normalized_delta);
            } else if response.dragged_by(PointerButton::Secondary) {
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
            *self.value = StereoSample::splat(default);
            response.mark_changed();
        }

        if ui.is_rect_visible(response.rect) {
            ui.painter().rect_filled(response.rect, 0.0, BG_COLOR);

            let lr_rect = response.rect.split_top_bottom_at_fraction(0.5);
            let paint_bar = |mut rect: Rect, norm_value: Sample| {
                if norm_value < 0.0 {
                    *rect.left_mut() += (1.0 + norm_value) * response.rect.width();
                    ui.painter().rect_filled(rect, 0.0, NEGATIVE_LEVEL_COLOR);
                } else {
                    *rect.right_mut() -= (1.0 - norm_value) * response.rect.width();
                    ui.painter().rect_filled(rect, 0.0, self.color);
                }
            };

            paint_bar(lr_rect.0, normalized_value.left());
            paint_bar(lr_rect.1, normalized_value.right());
        }

        let mut parts: Vec<String> = Vec::with_capacity(4);

        if self.value.left() != self.value.right() {
            parts.push(format!(
                "(L: {}, R: {})",
                self.format_value(self.value.left()),
                self.format_value(self.value.right())
            ));
        } else {
            parts.push(self.format_value(self.value.left()));
        }

        let label = parts.join(" ") + self.units.unwrap_or_default();

        ui.label(&label);
        response = response.on_hover_text_at_pointer(label);

        response
    }
}

impl Widget for StereoSlider<'_> {
    fn ui(mut self, ui: &mut Ui) -> Response {
        ui.horizontal_centered(|ui| self.add_contents(ui)).inner
    }
}
