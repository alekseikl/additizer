use egui::{Color32, PointerButton, Rect, Response, Sense, Ui, Widget, vec2};

use crate::{
    synth_engine::{Sample, StereoSample},
    utils::{db_to_gain, gain_to_db},
};

const BG_COLOR: Color32 = Color32::from_rgb(0, 0, 0);
const ATTENUATED_COLOR: Color32 = Color32::from_rgb(0x0b, 0x42, 0x67);
const AMPLIFIED_COLOR: Color32 = Color32::from_rgb(0x72, 0x12, 0x12);
const PHASE_COLOR: Color32 = Color32::from_rgb(0x42, 0x0b, 0x67);
const SLIDER_WIDTH: f32 = 12.0;
const MIN_DBS: Sample = -48.0;
const MAX_DBS: Sample = 24.0;
const DB_RANGE: Sample = MAX_DBS - MIN_DBS;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum BinSliderMode {
    #[default]
    Amplitude,
    Phase,
}

impl BinSliderMode {
    pub const ALL: &[Self] = &[Self::Amplitude, Self::Phase];

    pub fn label(self) -> &'static str {
        match self {
            Self::Amplitude => "Amplitudes",
            Self::Phase => "Phases",
        }
    }
}

pub struct BinSlider<'a> {
    label: Option<&'a str>,
    value: &'a mut StereoSample,
    width: f32,
    height: Option<f32>,
    mode: BinSliderMode,
    default: Option<StereoSample>,
}

impl<'a> BinSlider<'a> {
    pub fn new(value: &'a mut StereoSample) -> Self {
        Self {
            width: SLIDER_WIDTH,
            height: None,
            label: None,
            value,
            mode: BinSliderMode::Amplitude,
            default: None,
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

    pub fn mode(mut self, mode: BinSliderMode) -> Self {
        self.mode = mode;
        self
    }

    pub fn default(mut self, value: StereoSample) -> Self {
        self.default = Some(value);
        self
    }

    fn db_to_normalized(dbs: Sample) -> Sample {
        ((dbs - MIN_DBS) / DB_RANGE).clamp(0.0, 1.0)
    }

    fn gain_to_normalized(gain: Sample) -> Sample {
        if gain <= 0.0 {
            0.0
        } else {
            Self::db_to_normalized(gain_to_db(gain))
        }
    }

    fn normalized_to_gain(norm: Sample) -> Sample {
        let norm = norm.clamp(0.0, 1.0);

        if norm <= 0.0 {
            0.0
        } else {
            db_to_gain(MIN_DBS + norm * DB_RANGE)
        }
    }

    fn value_to_normalized(&self, value: Sample) -> Sample {
        match self.mode {
            BinSliderMode::Amplitude => Self::gain_to_normalized(value),
            BinSliderMode::Phase => value.clamp(0.0, 1.0),
        }
    }

    fn normalized_to_value(&self, norm: Sample) -> Sample {
        match self.mode {
            BinSliderMode::Amplitude => Self::normalized_to_gain(norm),
            BinSliderMode::Phase => norm.clamp(0.0, 1.0),
        }
    }

    fn fill_rect(&self, ui: &mut Ui, value: f32, rect: Rect) {
        let height = rect.height();

        match self.mode {
            BinSliderMode::Amplitude => {
                let norm_value = Self::gain_to_normalized(value);
                let zero_db_norm = Self::db_to_normalized(0.0);

                if norm_value > zero_db_norm {
                    ui.painter().rect_filled(
                        Rect::from_min_max(
                            rect.min + vec2(0.0, (1.0 - norm_value) * height),
                            rect.max,
                        ),
                        0.0,
                        AMPLIFIED_COLOR,
                    );
                    ui.painter().rect_filled(
                        Rect::from_min_max(
                            rect.min + vec2(0.0, (1.0 - zero_db_norm) * height),
                            rect.max,
                        ),
                        0.0,
                        ATTENUATED_COLOR,
                    );
                } else {
                    ui.painter().rect_filled(
                        Rect::from_min_max(
                            rect.min + vec2(0.0, (1.0 - norm_value) * height),
                            rect.max,
                        ),
                        0.0,
                        ATTENUATED_COLOR,
                    );
                }
            }
            BinSliderMode::Phase => {
                let norm_value = value.clamp(0.0, 1.0);

                ui.painter().rect_filled(
                    Rect::from_min_max(rect.min + vec2(0.0, (1.0 - norm_value) * height), rect.max),
                    0.0,
                    PHASE_COLOR,
                );
            }
        }
    }

    fn updated_value(&self, normalized_delta: f32, value: Sample) -> Sample {
        self.normalized_to_value(
            (self.value_to_normalized(value) + normalized_delta).clamp(0.0, 1.0),
        )
    }

    fn gain_to_db_string(gain: f32) -> String {
        if gain <= 0.0 {
            return "-Inf dB".to_string();
        }

        let dbs = gain_to_db(gain);

        if dbs <= MIN_DBS {
            "-Inf dB".to_string()
        } else if dbs == 0.0 {
            "0 dB".to_string()
        } else {
            format!("{:+.1} dB", dbs)
        }
    }

    fn value_string(&self, value: f32) -> String {
        match self.mode {
            BinSliderMode::Amplitude => Self::gain_to_db_string(value),
            BinSliderMode::Phase => format!("{:.0}%", value.clamp(0.0, 1.0) * 100.0),
        }
    }

    fn is_default_value(&self, value: Sample) -> bool {
        (value - self.reset_value().left()).abs() <= Sample::EPSILON
    }

    fn reset_value(&self) -> StereoSample {
        self.default.unwrap_or_else(|| match self.mode {
            BinSliderMode::Amplitude => StereoSample::splat(1.0),
            BinSliderMode::Phase => StereoSample::splat(0.0),
        })
    }

    fn clear_value(&self) -> StereoSample {
        match self.mode {
            BinSliderMode::Amplitude => StereoSample::splat(0.0),
            BinSliderMode::Phase => StereoSample::splat(0.5),
        }
    }

    fn handle_dragging(&mut self, ui: &mut Ui, response: &mut Response, normalized_delta: Sample) {
        if response.dragged_by(PointerButton::Primary) {
            self.value
                .set_left(self.updated_value(normalized_delta, self.value.left()));
            self.value
                .set_right(self.updated_value(normalized_delta, self.value.right()));
            response.mark_changed();
        } else if response.dragged_by(PointerButton::Secondary) {
            let is_right_channel = ui.memory(|mem| mem.data.get_temp(response.id).unwrap_or(false));

            if is_right_channel {
                self.value
                    .set_right(self.updated_value(normalized_delta, self.value.right()));
            } else {
                self.value
                    .set_left(self.updated_value(normalized_delta, self.value.left()));
            }
            response.mark_changed();
        }
    }

    fn handle_primary_click(&mut self, response: &mut Response) {
        *self.value = self.reset_value();
        response.mark_changed();
    }

    fn handle_secondary_click(&mut self, response: &mut Response) {
        *self.value = self.clear_value();
        response.mark_changed();
    }

    fn add_contents(&mut self, ui: &mut Ui) -> Response {
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
            let mut normalized_delta = -response.drag_delta().y / response.rect.height();

            if ui.input(|state| state.modifiers.shift) {
                normalized_delta *= 0.01;
            }

            self.handle_dragging(ui, &mut response, normalized_delta);
        } else if response.double_clicked_by(PointerButton::Primary) {
            self.handle_primary_click(&mut response);
        } else if response.double_clicked_by(PointerButton::Secondary) {
            self.handle_secondary_click(&mut response);
        } else if let Some(hover_pos) = response.hover_pos() {
            if modifiers.ctrl {
                *self.value = self.reset_value();
                response.mark_changed();
            } else if modifiers.alt {
                let value = self.normalized_to_value(
                    (1.0 - (hover_pos.y - response.rect.top()) / response.rect.height())
                        .clamp(0.0, 1.0),
                );

                *self.value = StereoSample::splat(value);
                response.mark_changed();
            }
        }

        if ui.is_rect_visible(response.rect) {
            let lr_rect = response.rect.split_left_right_at_fraction(0.5);

            ui.painter().rect_filled(response.rect, 0.0, BG_COLOR);
            self.fill_rect(ui, self.value.left(), lr_rect.0);
            self.fill_rect(ui, self.value.right(), lr_rect.1);

            let mut parts: Vec<String> = Vec::with_capacity(2);

            if let Some(label) = self.label {
                parts.push(label.to_string());
            }

            if self.value.left() != self.value.right() {
                parts.push(format!(
                    "L: {}\nR: {}",
                    self.value_string(self.value.left()),
                    self.value_string(self.value.right())
                ));
            } else if !self.is_default_value(self.value.left()) {
                parts.push(self.value_string(self.value.left()));
            }

            response = response.on_hover_text_at_pointer(parts.join("\n"));
        }

        response
    }
}

impl Widget for BinSlider<'_> {
    fn ui(mut self, ui: &mut Ui) -> Response {
        self.add_contents(ui)
    }
}
