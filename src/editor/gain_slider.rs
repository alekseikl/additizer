use egui_baseview::egui::{Color32, PointerButton, Rect, Response, Sense, Ui, Widget, pos2, vec2};
use nih_plug::util::MINUS_INFINITY_DB;

use crate::synth_engine::{Sample, StereoSample};

const BG_COLOR: Color32 = Color32::from_rgb(0, 0, 0);
const ATTENUATED_COLOR: Color32 = Color32::from_rgb(0x0b, 0x42, 0x67);
const AMPLIFIED_COLOR: Color32 = Color32::from_rgb(0x72, 0x12, 0x12);
const SLIDER_WIDTH: f32 = 12.0;

pub struct GainSlider<'a> {
    label: Option<&'a str>,
    value: &'a mut StereoSample,
    max_dbs: Sample,
    mid_point: Sample,
    skew_factor: Sample,
    horizontal: bool,
    width: f32,
    height: Option<f32>,
    color: Color32,
}

impl<'a> GainSlider<'a> {
    pub fn new(value: &'a mut StereoSample) -> Self {
        Self {
            max_dbs: 48.0,
            mid_point: 0.75,
            skew_factor: 1.6,
            horizontal: false,
            width: SLIDER_WIDTH,
            height: None,
            color: ATTENUATED_COLOR,
            label: None,
            value,
        }
    }

    pub fn label(mut self, label: &'a str) -> Self {
        self.label = Some(label);
        self
    }

    pub fn height(mut self, height: f32) -> Self {
        self.height = Some(height);
        self
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    #[allow(unused)]
    pub fn horizontal(mut self) -> Self {
        self.horizontal = true;
        self
    }

    #[allow(unused)]
    pub fn color(mut self, color: Color32) -> Self {
        self.color = color;
        self
    }

    #[allow(unused)]
    pub fn max_dbs(mut self, max_dbs: Sample) -> Self {
        self.max_dbs = max_dbs;
        self
    }

    #[allow(unused)]
    pub fn mid_point(mut self, mid_point: Sample) -> Self {
        self.mid_point = mid_point;
        self
    }

    #[allow(unused)]
    pub fn skew(mut self, skew_factor: Sample) -> Self {
        self.skew_factor = skew_factor;
        self
    }

    fn gain_to_normalized(&self, gain: f32) -> f32 {
        let dbs = nih_plug::util::gain_to_db(gain);

        if dbs > 0.0 {
            let normalized = dbs / self.max_dbs;

            self.mid_point + (1.0 - self.mid_point) * normalized.powf(self.skew_factor.recip())
        } else {
            let normalized = dbs / nih_plug::util::MINUS_INFINITY_DB;

            self.mid_point * (1.0 - normalized.powf(self.skew_factor.recip()))
        }
    }

    fn normalized_to_gain(&self, norm: f32) -> f32 {
        let dbs = if norm > self.mid_point {
            let normalized = (norm - self.mid_point) / (1.0 - self.mid_point);

            self.max_dbs * normalized.powf(self.skew_factor)
        } else {
            let normalized = 1.0 - norm / self.mid_point;

            nih_plug::util::MINUS_INFINITY_DB * normalized.powf(self.skew_factor)
        };

        nih_plug::util::db_to_gain(dbs)
    }

    fn fill_gain_rect(&self, ui: &mut Ui, gain: f32, rect: Rect) {
        let norm_value = self.gain_to_normalized(gain);
        let height = rect.height();

        if norm_value > self.mid_point {
            ui.painter().rect_filled(
                Rect::from_min_max(rect.min + vec2(0.0, (1.0 - norm_value) * height), rect.max),
                0.0,
                AMPLIFIED_COLOR,
            );
            ui.painter().rect_filled(
                Rect::from_min_max(
                    rect.min + vec2(0.0, (1.0 - self.mid_point) * height),
                    rect.max,
                ),
                0.0,
                self.color,
            );
        } else {
            ui.painter().rect_filled(
                Rect::from_min_max(rect.min + vec2(0.0, (1.0 - norm_value) * height), rect.max),
                0.0,
                self.color,
            );
        }
    }

    fn fill_gain_rect_horizontal(&self, ui: &mut Ui, gain: f32, rect: Rect) {
        let norm_value = self.gain_to_normalized(gain);
        let width = rect.width();

        if norm_value > self.mid_point {
            ui.painter().rect_filled(
                Rect::from_min_max(rect.min, pos2(rect.min.x + norm_value * width, rect.max.y)),
                0.0,
                AMPLIFIED_COLOR,
            );
            ui.painter().rect_filled(
                Rect::from_min_max(
                    rect.min,
                    pos2(rect.min.x + self.mid_point * width, rect.max.y),
                ),
                0.0,
                self.color,
            );
        } else {
            ui.painter().rect_filled(
                Rect::from_min_max(rect.min, pos2(rect.min.x + norm_value * width, rect.max.y)),
                0.0,
                self.color,
            );
        }
    }

    fn updated_gain(&self, normalized_delta: f32, gain: Sample) -> Sample {
        self.normalized_to_gain((self.gain_to_normalized(gain) + normalized_delta).clamp(0.0, 1.0))
    }

    fn gain_to_db_string(gain: f32) -> String {
        let dbs = nih_plug::util::gain_to_db(gain);
        if dbs <= MINUS_INFINITY_DB {
            "-Inf dB".to_string()
        } else if dbs == 0.0 {
            "0 dB".to_string()
        } else {
            format!("{:+.1} dB", nih_plug::util::gain_to_db(gain))
        }
    }

    fn handle_dragging(&mut self, ui: &mut Ui, response: &mut Response, normalized_delta: Sample) {
        if response.dragged_by(PointerButton::Primary) {
            self.value
                .set_left(self.updated_gain(normalized_delta, self.value.left()));
            self.value
                .set_right(self.updated_gain(normalized_delta, self.value.right()));
            response.mark_changed();
        } else if response.dragged_by(PointerButton::Secondary) {
            let is_right_channel = ui.memory(|mem| mem.data.get_temp(response.id).unwrap_or(false));

            if is_right_channel {
                self.value
                    .set_right(self.updated_gain(normalized_delta, self.value.right()));
            } else {
                self.value
                    .set_left(self.updated_gain(normalized_delta, self.value.left()));
            }
            response.mark_changed();
        }
    }

    fn handle_primary_click(&mut self, response: &mut Response) {
        *self.value = StereoSample::splat(1.0);
        response.mark_changed();
    }

    fn handle_secondary_click(&mut self, response: &mut Response) {
        *self.value = StereoSample::splat(0.0);
        response.mark_changed();
    }

    fn add_contents_vertical(&mut self, ui: &mut Ui) -> Response {
        let mut response = ui.allocate_response(
            vec2(self.width, self.height.unwrap_or(ui.available_size().y)),
            Sense::click_and_drag(),
        );

        if let Some(pos) = response.interact_pointer_pos()
            && response.drag_started_by(PointerButton::Secondary)
        {
            let is_right_channel = pos.x >= response.rect.center().x;

            ui.memory_mut(|mem| mem.data.insert_temp(response.id, is_right_channel));
        }

        let modifiers = ui.input(|state| state.modifiers);

        if response.dragged() {
            let normalized_delta = -response.drag_delta().y / response.rect.height();

            self.handle_dragging(ui, &mut response, normalized_delta);
        } else if response.double_clicked_by(PointerButton::Primary) {
            self.handle_primary_click(&mut response);
        } else if response.double_clicked_by(PointerButton::Secondary) {
            self.handle_secondary_click(&mut response);
        } else if let Some(hover_pos) = response.hover_pos() {
            if modifiers.ctrl {
                *self.value = StereoSample::splat(1.0);
                response.mark_changed();
            } else if modifiers.alt {
                let gain = self.normalized_to_gain(
                    (1.0 - (hover_pos.y - response.rect.top()) / response.rect.height())
                        .clamp(0.0, 1.0),
                );

                *self.value = StereoSample::splat(gain);
                response.mark_changed();
            }
        }

        if ui.is_rect_visible(response.rect) {
            let lr_rect = response.rect.split_left_right_at_fraction(0.5);

            ui.painter().rect_filled(response.rect, 0.0, BG_COLOR);
            self.fill_gain_rect(ui, self.value.left(), lr_rect.0);
            self.fill_gain_rect(ui, self.value.right(), lr_rect.1);

            let mut parts: Vec<String> = Vec::with_capacity(2);

            if let Some(label) = self.label {
                parts.push(label.to_string());
            }

            if self.value.left() != self.value.right() {
                parts.push(format!(
                    "L: {}\nR: {}",
                    Self::gain_to_db_string(self.value.left()),
                    Self::gain_to_db_string(self.value.right())
                ));
            } else if (self.value.left() - 1.0).abs() > Sample::EPSILON {
                parts.push(Self::gain_to_db_string(self.value.left()));
            }

            response = response.on_hover_text_at_pointer(parts.join("\n"));
        }

        response
    }

    fn add_contents_horizontal(&mut self, ui: &mut Ui) -> Response {
        let mut response = ui.allocate_response(
            vec2(self.height.unwrap_or(ui.available_size().x), self.width),
            Sense::click_and_drag(),
        );

        if let Some(pos) = response.interact_pointer_pos()
            && response.drag_started_by(PointerButton::Secondary)
        {
            let is_right_channel = pos.y >= response.rect.center().y;

            ui.memory_mut(|mem| mem.data.insert_temp(response.id, is_right_channel));
        }

        if response.dragged() {
            let normalized_delta = response.drag_delta().x / response.rect.width();

            self.handle_dragging(ui, &mut response, normalized_delta);
        } else if response.double_clicked_by(PointerButton::Primary) {
            self.handle_primary_click(&mut response);
        } else if response.double_clicked_by(PointerButton::Secondary) {
            self.handle_secondary_click(&mut response);
        }

        let label = if self.value.left() != self.value.right() {
            format!(
                "L: {}, R: {}",
                Self::gain_to_db_string(self.value.left()),
                Self::gain_to_db_string(self.value.right())
            )
        } else {
            Self::gain_to_db_string(self.value.left())
        };

        if ui.is_rect_visible(response.rect) {
            let lr_rect = response.rect.split_top_bottom_at_fraction(0.5);

            ui.painter().rect_filled(response.rect, 0.0, BG_COLOR);
            self.fill_gain_rect_horizontal(ui, self.value.left(), lr_rect.0);
            self.fill_gain_rect_horizontal(ui, self.value.right(), lr_rect.1);

            response = response.on_hover_text_at_pointer(&label);
        }

        ui.label(&label);

        response
    }
}

impl Widget for GainSlider<'_> {
    fn ui(mut self, ui: &mut Ui) -> Response {
        if self.horizontal {
            ui.horizontal(|ui| self.add_contents_horizontal(ui)).inner
        } else {
            self.add_contents_vertical(ui)
        }
    }
}
