use egui_baseview::egui::{Color32, PointerButton, Rect, Response, Sense, Ui, Widget, pos2, vec2};

use crate::synth_engine::{Sample, StereoSample};

const BG_COLOR: Color32 = Color32::from_rgb(0, 0, 0);
const LEVEL_COLOR: Color32 = Color32::from_rgb(0x0b, 0x42, 0x67);
const AMPLIFIED_COLOR: Color32 = Color32::from_rgb(0x72, 0x12, 0x12);
const SLIDER_HEIGHT: f32 = 16.0;
const MIN_DBS: f32 = -100.0;

pub struct DbSlider<'a> {
    value: &'a mut StereoSample,
    max_dbs: Sample,
    mid_point: Sample,
    skew_factor: Sample,
    width: f32,
}

impl<'a> DbSlider<'a> {
    pub fn new(value: &'a mut StereoSample) -> Self {
        Self {
            value,
            max_dbs: 24.0,
            mid_point: 0.7,
            skew_factor: 1.6,
            width: 200.0,
        }
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    #[allow(unused)]
    pub fn mid_point(mut self, mid_point: Sample) -> Self {
        self.mid_point = mid_point;
        self
    }

    #[allow(unused)]
    pub fn max_dbs(mut self, max_dbs: Sample) -> Self {
        self.max_dbs = max_dbs;
        self
    }

    #[allow(unused)]
    pub fn skew(mut self, skew_factor: Sample) -> Self {
        self.skew_factor = skew_factor;
        self
    }

    fn db_to_normalized(&self, dbs: Sample) -> Sample {
        let dbs = dbs.clamp(MIN_DBS, self.max_dbs);

        if dbs < 0.0 {
            (1.0 - (dbs / MIN_DBS).powf(self.skew_factor.recip())) * self.mid_point
        } else {
            self.mid_point
                + (dbs / self.max_dbs).powf(self.skew_factor.recip()) * (1.0 - self.mid_point)
        }
    }

    fn normalized_to_db(&self, normalized: Sample) -> Sample {
        let normalized = normalized.clamp(0.0, 1.0);

        if normalized < self.mid_point {
            (1.0 - (normalized / self.mid_point)).powf(self.skew_factor) * MIN_DBS
        } else {
            ((normalized - self.mid_point) / (1.0 - self.mid_point)).powf(self.skew_factor)
                * self.max_dbs
        }
    }

    fn fill_rect(&self, ui: &mut Ui, norm_value: f32, rect: Rect) {
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
                LEVEL_COLOR,
            );
        } else {
            ui.painter().rect_filled(
                Rect::from_min_max(rect.min, pos2(rect.min.x + norm_value * width, rect.max.y)),
                0.0,
                LEVEL_COLOR,
            );
        }
    }

    fn update_normalized_value(&mut self, response: &mut Response, normalized: StereoSample) {
        *self.value = normalized
            .iter()
            .map(|norm| self.normalized_to_db(*norm))
            .collect();
        response.mark_changed();
    }

    fn format_dbs(dbs: f32) -> String {
        if dbs <= MIN_DBS {
            "-Inf".to_string()
        } else if dbs == 0.0 {
            "0".to_string()
        } else {
            format!("{:+.1}", dbs)
        }
    }

    fn add_contents(&mut self, ui: &mut Ui) -> Response {
        let mut response =
            ui.allocate_response(vec2(self.width, SLIDER_HEIGHT), Sense::click_and_drag());

        let normalized_value: StereoSample = self
            .value
            .iter()
            .map(|dbs| self.db_to_normalized(*dbs))
            .collect();

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

                let normalized_delta = if is_right_channel {
                    StereoSample::new(0.0, normalized_delta)
                } else {
                    StereoSample::new(normalized_delta, 0.0)
                };
                self.update_normalized_value(&mut response, normalized_value + normalized_delta);
            }
        } else if response.double_clicked_by(PointerButton::Primary) {
            *self.value = 0.0.into();
            response.mark_changed();
        } else if response.double_clicked_by(PointerButton::Secondary) {
            self.update_normalized_value(&mut response, 0.0.into());
        }

        if ui.is_rect_visible(response.rect) {
            let lr_rect = response.rect.split_top_bottom_at_fraction(0.5);

            ui.painter().rect_filled(response.rect, 0.0, BG_COLOR);

            self.fill_rect(ui, normalized_value.left(), lr_rect.0);
            self.fill_rect(ui, normalized_value.right(), lr_rect.1);
        }

        let mut parts: Vec<String> = Vec::with_capacity(4);

        if self.value.left() != self.value.right() {
            parts.push(format!(
                "(L: {}, R: {})",
                Self::format_dbs(self.value.left()),
                Self::format_dbs(self.value.right())
            ));
        } else {
            parts.push(Self::format_dbs(self.value.left()));
        }

        let label = format!("{} dB", parts.join(" "));

        ui.label(&label);
        response = response.on_hover_text_at_pointer(label);

        response
    }
}

impl Widget for DbSlider<'_> {
    fn ui(mut self, ui: &mut Ui) -> Response {
        ui.horizontal(|ui| self.add_contents(ui)).inner
    }
}
