use std::ops::RangeInclusive;

use egui::{Color32, PointerButton, Pos2, Rect, Response, Sense, Ui, Vec2, Widget, vec2};

use crate::synth_engine::{Sample, StereoSample};

const BG_COLOR: Color32 = Color32::from_rgb(0, 0, 0);
const LEVEL_COLOR: Color32 = Color32::from_rgb(0x0b, 0x42, 0x67);
const NEGATIVE_LEVEL_COLOR: Color32 = Color32::from_rgb(0x72, 0x72, 0x12);
const MODULATED_COLOR: Color32 = Color32::from_rgb(0x9a, 0x6a, 0x12);

pub struct StereoSlider<'a> {
    units: Option<&'a str>,
    value: &'a mut StereoSample,
    modulated: Option<StereoSample>,
    range: RangeInclusive<Sample>,
    default: Option<Sample>,
    precision: usize,
    skew_factor: Sample,
    display_scale_factor: Sample,
    length: f32,
    thickness: f32,
    vertical: bool,
    color: Color32,
    allow_inverse: bool,
}

impl<'a> StereoSlider<'a> {
    pub fn new(value: &'a mut StereoSample) -> Self {
        Self {
            skew_factor: 1.0,
            display_scale_factor: 1.0,
            length: 200.0,
            thickness: 16.0,
            vertical: false,
            color: LEVEL_COLOR,
            units: None,
            default: None,
            precision: 1,
            value,
            modulated: None,
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

    pub fn length(mut self, length: f32) -> Self {
        self.length = length;
        self
    }

    pub fn thickness(mut self, thickness: f32) -> Self {
        self.thickness = thickness;
        self
    }

    pub fn vertical(mut self) -> Self {
        self.vertical = true;
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

    pub fn modulated(mut self, value: Option<StereoSample>) -> Self {
        self.modulated = value;
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

    fn normalized_value_from(&self, value: &StereoSample) -> StereoSample {
        let start = *self.range.start();
        let end = *self.range.end();
        let min_normalized = self.normalized_minimum();
        let clamped = ((*value - start) * (end - start).recip()).clamp(min_normalized, 1.0);

        self.skew_value(clamped)
    }

    fn normalized_value(&self) -> StereoSample {
        self.normalized_value_from(self.value)
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

    fn response_size(&self) -> Vec2 {
        if self.vertical {
            vec2(self.thickness, self.length)
        } else {
            vec2(self.length, self.thickness)
        }
    }

    fn normalized_delta(&self, response: &Response) -> f32 {
        if self.vertical {
            -response.drag_delta().y / response.rect.height()
        } else {
            response.drag_delta().x / response.rect.width()
        }
    }

    fn is_right_channel(&self, pos: Pos2, response: &Response) -> bool {
        if self.vertical {
            pos.x >= response.rect.center().x
        } else {
            pos.y >= response.rect.center().y
        }
    }

    fn paint_bars(&self, ui: &mut Ui, response: &Response, normalized_value: StereoSample) {
        if self.vertical {
            let lr_rect = response.rect.split_left_right_at_fraction(0.5);
            let paint_bar = |mut rect: Rect, norm_value: Sample| {
                if norm_value < 0.0 {
                    *rect.bottom_mut() -= (1.0 + norm_value) * response.rect.height();
                    ui.painter().rect_filled(rect, 0.0, NEGATIVE_LEVEL_COLOR);
                } else {
                    *rect.top_mut() += (1.0 - norm_value) * response.rect.height();
                    ui.painter().rect_filled(rect, 0.0, self.color);
                }
            };

            paint_bar(lr_rect.0, normalized_value.left());
            paint_bar(lr_rect.1, normalized_value.right());
        } else {
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
    }

    fn paint_modulated_indicators(&self, ui: &mut Ui, response: &Response) {
        let Some(modulated) = self.modulated else {
            return;
        };

        let normalized_modulated = self.normalized_value_from(&modulated);

        let paint_horizontal_bar = |rect: Rect, norm_value: Sample, at_top: bool| {
            let y = if at_top {
                rect.top()
            } else {
                rect.bottom() - 1.0
            };
            let bar_rect = if norm_value < 0.0 {
                let x = rect.left() + (1.0 + norm_value) * rect.width();
                Rect::from_min_max(Pos2::new(x, y), Pos2::new(rect.right(), y + 1.0))
            } else {
                let x = rect.left() + norm_value * rect.width();
                Rect::from_min_max(Pos2::new(rect.left(), y), Pos2::new(x, y + 1.0))
            };
            ui.painter().rect_filled(bar_rect, 0.0, MODULATED_COLOR);
        };

        let paint_vertical_bar = |rect: Rect, norm_value: Sample, at_left: bool| {
            let x = if at_left {
                rect.left()
            } else {
                rect.right() - 1.0
            };
            let bar_rect = if norm_value < 0.0 {
                let y = rect.bottom() - (1.0 + norm_value) * rect.height();
                Rect::from_min_max(Pos2::new(x, y), Pos2::new(x + 1.0, rect.bottom()))
            } else {
                let y = rect.bottom() - norm_value * rect.height();
                Rect::from_min_max(Pos2::new(x, y), Pos2::new(x + 1.0, rect.bottom()))
            };
            ui.painter().rect_filled(bar_rect, 0.0, MODULATED_COLOR);
        };

        if self.vertical {
            let lr_rect = response.rect.split_left_right_at_fraction(0.5);
            paint_vertical_bar(lr_rect.0, normalized_modulated.left(), true);
            paint_vertical_bar(lr_rect.1, normalized_modulated.right(), false);
        } else {
            let lr_rect = response.rect.split_top_bottom_at_fraction(0.5);
            paint_horizontal_bar(lr_rect.0, normalized_modulated.left(), true);
            paint_horizontal_bar(lr_rect.1, normalized_modulated.right(), false);
        }
    }

    fn add_contents(&mut self, ui: &mut Ui) -> Response {
        let mut response = ui.allocate_response(self.response_size(), Sense::click_and_drag());
        let normalized_value = self.normalized_value();

        if let Some(pos) = response.interact_pointer_pos()
            && response.drag_started_by(PointerButton::Secondary)
        {
            ui.memory_mut(|mem| {
                mem.data
                    .insert_temp(response.id, self.is_right_channel(pos, &response))
            });
        }

        if response.dragged() {
            let mut normalized_delta = self.normalized_delta(&response);

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
            self.paint_bars(ui, &response, normalized_value);
            self.paint_modulated_indicators(ui, &response);
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

        if !self.vertical {
            ui.label(&label);
        }
        response = response.on_hover_text_at_pointer(label);

        response
    }
}

impl Widget for StereoSlider<'_> {
    fn ui(mut self, ui: &mut Ui) -> Response {
        if self.vertical {
            self.add_contents(ui)
        } else {
            ui.horizontal_centered(|ui| self.add_contents(ui)).inner
        }
    }
}
