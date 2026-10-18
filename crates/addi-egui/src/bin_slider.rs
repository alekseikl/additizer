use egui::{
    Color32, FontFamily, FontId, Id, LayerId, Order, PointerButton, Pos2, Rect, Response, Sense,
    Stroke, StrokeKind, Ui, Widget, ecolor::Hsva, vec2,
};

use addi_dsp::{db_to_gain, gain_to_db};
use addi_engine::{MAX_LEVEL_DB, MIN_LEVEL_DB, Smp, StereoSample};

use super::utils::hsva;

const BG_COLOR: Color32 = Color32::from_rgb(0, 0, 0);
const ATTENUATED_COLOR: Color32 = Color32::from_rgb(0x0b, 0x42, 0x67);
const AMPLIFIED_COLOR: Color32 = Color32::from_rgb(0x72, 0x12, 0x12);
const PHASE_COLOR: Color32 = Color32::from_rgb(0x42, 0x0b, 0x67);
const SLIDER_WIDTH: f32 = 12.0;
const DB_RANGE: Smp = MAX_LEVEL_DB - MIN_LEVEL_DB;

const LABEL_BG_COLOR: Hsva = hsva(0.115, 0.05, 0.01, 1.0);
const LABEL_BORDER_COLOR: Hsva = hsva(0.115, 0.05, 0.2, 1.0);
const LABEL_TEXT_COLOR: Hsva = hsva(0.115, 0.05, 0.5, 1.0);
const LABEL_PADDING: f32 = 6.0;
const LABEL_MARGIN: f32 = 1.0;
const LABEL_POINTER_GAP: f32 = 2.0;
const LABEL_CORNER_RADIUS: f32 = 2.0;
const HOVER_DELAY: f64 = 0.5;

#[derive(Default, Clone)]
struct LabelState {
    visible: bool,
    hover_since: Option<f64>,
}

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

    fn db_to_normalized(dbs: Smp) -> Smp {
        ((dbs - MIN_LEVEL_DB) / DB_RANGE).clamp(0.0, 1.0)
    }

    fn gain_to_normalized(gain: Smp) -> Smp {
        if gain <= 0.0 {
            0.0
        } else {
            Self::db_to_normalized(gain_to_db(gain))
        }
    }

    fn normalized_to_gain(norm: Smp) -> Smp {
        let norm = norm.clamp(0.0, 1.0);

        if norm <= 0.0 {
            0.0
        } else {
            db_to_gain(MIN_LEVEL_DB + norm * DB_RANGE)
        }
    }

    fn value_to_normalized(&self, value: Smp) -> Smp {
        match self.mode {
            BinSliderMode::Amplitude => Self::gain_to_normalized(value),
            BinSliderMode::Phase => value.clamp(0.0, 1.0),
        }
    }

    fn normalized_to_value(&self, norm: Smp) -> Smp {
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

    fn updated_value(&self, normalized_delta: f32, value: Smp) -> Smp {
        self.normalized_to_value(
            (self.value_to_normalized(value) + normalized_delta).clamp(0.0, 1.0),
        )
    }

    fn gain_to_db_string(gain: f32) -> String {
        if gain <= 0.0 {
            return "-Inf dB".to_string();
        }

        let dbs = gain_to_db(gain);

        if dbs <= MIN_LEVEL_DB {
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

    fn is_default_value(&self, value: Smp) -> bool {
        (value - self.reset_value().left()).abs() <= Smp::EPSILON
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

    fn format_label(&self) -> Option<String> {
        if self.value.left() != self.value.right() {
            let value = format!(
                "L: {}\nR: {}",
                self.value_string(self.value.left()),
                self.value_string(self.value.right())
            );

            return Some(match self.label {
                Some(label) => format!("{label}\n{value}"),
                None => value,
            });
        }

        if self.is_default_value(self.value.left()) {
            return self.label.map(str::to_string);
        }

        let value = self.value_string(self.value.left());

        Some(match self.label {
            Some(label) => format!("{label}: {value}"),
            None => value,
        })
    }

    fn paint_label(&self, ui: &Ui, slider_rect: Rect, pointer_y: f32) {
        let Some(text) = self.format_label() else {
            return;
        };

        let text_color = Color32::from(LABEL_TEXT_COLOR);
        let bg_color = Color32::from(LABEL_BG_COLOR);
        let border_color = Color32::from(LABEL_BORDER_COLOR);

        let painter = ui.painter();
        let galley = painter.layout_no_wrap(
            text,
            FontId::new(11.0, FontFamily::Name("Bold".into())),
            text_color,
        );

        let padding = vec2(LABEL_PADDING, LABEL_PADDING * 0.5);
        let box_size = galley.size() + padding * 2.0;
        let bounds = ui.clip_rect();
        let vertical = bounds.y_range().intersection(slider_rect.y_range());

        let top = (pointer_y - box_size.y - LABEL_POINTER_GAP)
            .clamp(vertical.min, (vertical.max - box_size.y).max(vertical.min));
        let left_right = slider_rect.right() + LABEL_MARGIN;
        let left = if left_right + box_size.x <= bounds.right() {
            left_right
        } else {
            slider_rect.left() - box_size.x - LABEL_MARGIN
        };
        let left = left.clamp(
            bounds.left(),
            (bounds.right() - box_size.x).max(bounds.left()),
        );
        let box_rect = Rect::from_min_size(Pos2::new(left, top), box_size);

        let fg_painter = painter
            .clone()
            .with_layer_id(LayerId::new(Order::Foreground, Id::new("bin-slider-label")));
        fg_painter.rect(
            box_rect,
            LABEL_CORNER_RADIUS,
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

    fn handle_dragging(&mut self, ui: &mut Ui, response: &mut Response, normalized_delta: Smp) {
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
        }

        if self.label_visible(ui, &response)
            && let Some(pointer_y) = response
                .interact_pointer_pos()
                .or(response.hover_pos())
                .map(|pos| pos.y)
        {
            self.paint_label(ui, response.rect, pointer_y);
        }

        response
    }
}

impl Widget for BinSlider<'_> {
    fn ui(mut self, ui: &mut Ui) -> Response {
        self.add_contents(ui)
    }
}
