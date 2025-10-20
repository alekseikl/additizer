use egui_baseview::egui::{
    Color32, FontFamily, FontId, PointerButton, Pos2, Rect, Response, Sense, Stroke, StrokeKind,
    Ui, Widget, vec2,
};
use nih_plug::util::MINUS_INFINITY_DB;

use crate::synth_engine::types::{Sample, StereoValue};

const BG_COLOR: Color32 = Color32::from_rgb(0, 0, 0);
const BORDER_COLOR: Color32 = Color32::from_rgb(0x7f, 0x7f, 0x7f);
const ATTENUATED_COLOR: Color32 = Color32::from_rgb(0x0b, 0x42, 0x67);
const AMPLIFIED_COLOR: Color32 = Color32::from_rgb(0x72, 0x12, 0x12);
const SLIDER_WIDTH: f32 = 16.0;

enum LabelType {
    LeftChannel,
    RightChannel,
    BothChannels,
    Label(String),
}

pub struct GainSlider<'a> {
    label: Option<&'a str>,
    value: &'a mut StereoValue,
    max_dbs: Sample,
    mid_point: Sample,
    skew_factor: Sample,
    height: f32,
    color: Color32,
}

impl<'a> GainSlider<'a> {
    pub fn new(value: &'a mut StereoValue) -> Self {
        Self {
            max_dbs: 48.0,
            mid_point: 0.75,
            skew_factor: 1.4,
            height: 100.0,
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
        self.height = height;
        self
    }

    pub fn color(mut self, color: Color32) -> Self {
        self.color = color;
        self
    }

    pub fn max_dbs(mut self, max_dbs: Sample) -> Self {
        self.max_dbs = max_dbs;
        self
    }

    pub fn mid_point(mut self, mid_point: Sample) -> Self {
        self.mid_point = mid_point;
        self
    }

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

    fn updated_gain(&self, normalized_delta: f32, gain: Sample) -> Sample {
        self.normalized_to_gain((self.gain_to_normalized(gain) + normalized_delta).clamp(0.0, 1.0))
    }

    fn gain_to_db_string(gain: f32) -> String {
        let dbs = nih_plug::util::gain_to_db(gain);
        if dbs <= MINUS_INFINITY_DB {
            "-Inf".to_string()
        } else {
            format!("{:+.1}", nih_plug::util::gain_to_db(gain))
        }
    }

    fn add_contents(&mut self, ui: &mut Ui) -> Response {
        let mut response = ui.allocate_response(
            vec2(SLIDER_WIDTH, self.height),
            Sense::click_and_drag() | Sense::hover(),
        );
        let mut label_type: Option<LabelType> = None;

        if let Some(pos) = response.interact_pointer_pos()
            && response.drag_started_by(PointerButton::Secondary)
        {
            let is_right_channel = pos.x >= response.rect.center().x;

            ui.memory_mut(|mem| mem.data.insert_temp(response.id, is_right_channel));
        }

        let modifiers = ui.input(|state| state.modifiers);

        if response.dragged() {
            let normalized_delta = -response.drag_delta().y / response.rect.height();

            if response.dragged_by(PointerButton::Primary) {
                self.value.left = self.updated_gain(normalized_delta, self.value.left);
                self.value.right = self.updated_gain(normalized_delta, self.value.right);
                label_type = Some(LabelType::BothChannels);
                response.mark_changed();
            } else if response.dragged_by(PointerButton::Secondary) {
                let is_right_channel =
                    ui.memory(|mem| mem.data.get_temp(response.id).unwrap_or(false));

                if is_right_channel {
                    self.value.right = self.updated_gain(normalized_delta, self.value.right);
                    label_type = Some(LabelType::RightChannel);
                } else {
                    self.value.left = self.updated_gain(normalized_delta, self.value.left);
                    label_type = Some(LabelType::LeftChannel);
                }
                response.mark_changed();
            }
        } else if response.double_clicked_by(PointerButton::Primary) {
            *self.value = StereoValue::mono(1.0);
            response.mark_changed();
        } else if response.double_clicked_by(PointerButton::Secondary) {
            *self.value = StereoValue::mono(0.0);
            response.mark_changed();
        } else if let Some(hover_pos) = response.hover_pos() {
            if modifiers.ctrl {
                *self.value = StereoValue::mono(1.0);
                response.mark_changed();
            } else if modifiers.alt {
                let gain = self.normalized_to_gain(
                    (1.0 - (hover_pos.y - response.rect.top()) / response.rect.height())
                        .clamp(0.0, 1.0),
                );

                *self.value = StereoValue::mono(gain);
                response.mark_changed();
            }

            if let Some(label) = self.label {
                label_type = Some(LabelType::Label(label.to_string()));
            }
        }

        if ui.is_rect_visible(response.rect) {
            let lr_rect = response.rect.split_left_right_at_fraction(0.5);

            ui.painter().rect_filled(response.rect, 0.0, BG_COLOR);
            self.fill_gain_rect(ui, self.value.left, lr_rect.0);
            self.fill_gain_rect(ui, self.value.right, lr_rect.1);

            ui.painter().rect_stroke(
                response.rect,
                0.0,
                Stroke::new(1.0, BORDER_COLOR),
                StrokeKind::Inside,
            );

            if let Some(label_type) = label_type {
                let label_text = match label_type {
                    LabelType::LeftChannel => {
                        format!("L:{}", Self::gain_to_db_string(self.value.left))
                    }
                    LabelType::RightChannel => {
                        format!("R:{}", Self::gain_to_db_string(self.value.right))
                    }
                    LabelType::BothChannels => {
                        if self.value.left == self.value.right {
                            Self::gain_to_db_string(self.value.left)
                        } else {
                            format!(
                                "L:{}\nR:{}",
                                Self::gain_to_db_string(self.value.left),
                                Self::gain_to_db_string(self.value.right)
                            )
                        }
                    }
                    LabelType::Label(label) => label,
                };

                let window_width = ui.input(|i| i.content_rect()).width();

                let label_padding = vec2(3.0, 2.0);
                let font = FontId::new(9.0, FontFamily::Monospace);
                let text_galley = ui.painter().layout_no_wrap(label_text, font, BORDER_COLOR);
                let mut text_rect = text_galley.rect;

                let mut label_box = Rect::from_min_max(
                    Pos2::ZERO,
                    Pos2::new(
                        (text_rect.width() + 2.0 * label_padding.x).max(response.rect.width()),
                        text_rect.height() + 2.0 * label_padding.y - 1.0,
                    ),
                );

                label_box = label_box.translate(vec2(
                    (response.rect.center().x - 0.5 * label_box.width())
                        .clamp(0.0, window_width - label_box.width()),
                    response.rect.bottom(),
                ));

                text_rect.set_center(label_box.center());

                ui.painter().rect(
                    label_box,
                    0.0,
                    BG_COLOR,
                    Stroke::new(1.0, BORDER_COLOR),
                    StrokeKind::Inside,
                );
                ui.painter()
                    .galley(text_rect.min, text_galley, Color32::WHITE);
            }
        }

        response
    }
}

impl Widget for GainSlider<'_> {
    fn ui(mut self, ui: &mut Ui) -> Response {
        self.add_contents(ui)
    }
}
