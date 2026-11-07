use egui::{
    Align2, Area, FontFamily, FontId, Id, Key, Margin, Order, Pos2, Rect, Stroke, TextEdit, Ui,
    ecolor::Hsva, pos2, text::CCursorRange,
};

use addi_engine::Smp;

use crate::units::Units;
use crate::utils::hsva;

const BG: Hsva = hsva(0.115, 0.05, 0.01, 1.0);
const BORDER: Hsva = hsva(0.115, 0.05, 0.2, 1.0);
const TEXT: Hsva = hsva(0.115, 0.05, 0.5, 1.0);
const PADDING: f32 = 6.0;
const WIDTH: f32 = 160.0;
/// Half the field width, used to keep the field inside the screen.
const REACH: f32 = 80.0;

/// Text typed into a value field. `units` formats the starting value and parses the result.
#[derive(Clone)]
pub(crate) struct ValueEntry {
    units: Units,
    text: String,
    request_focus: bool,
}

impl ValueEntry {
    pub(crate) fn new(units: Units, value: Smp) -> Self {
        Self {
            units,
            text: units.format_input(value),
            request_focus: true,
        }
    }

    /// Foreground text field. Enter or a click away commits a parsed value.
    /// Escape cancels. Enter on text that does not parse keeps the field open.
    pub(crate) fn show(&mut self, ui: &Ui, id: Id, pivot: Align2, pos: Pos2) -> ValueEntryEvent {
        let mut output = Area::new(id.with("area"))
            .order(Order::Foreground)
            .fixed_pos(pos)
            .pivot(pivot)
            .show(ui.ctx(), |ui| {
                let border = Stroke::new(1.0, BORDER);

                ui.visuals_mut().widgets.inactive.bg_stroke = border;
                ui.visuals_mut().widgets.hovered.bg_stroke = border;
                ui.visuals_mut().widgets.active.bg_stroke = border;

                TextEdit::singleline(&mut self.text)
                    .id(id.with("text"))
                    .desired_width(WIDTH)
                    .font(FontId::new(11.0, FontFamily::Name("Bold".into())))
                    .text_color(TEXT.into())
                    .background_color(BG.into())
                    .margin(Margin::symmetric(PADDING as i8, (PADDING * 0.5) as i8))
                    .show(ui)
            })
            .inner;

        if self.request_focus {
            output.response.request_focus();
            output
                .state
                .cursor
                .set_char_range(Some(CCursorRange::one(output.galley.end())));
            output.state.store(ui.ctx(), output.response.id);
            self.request_focus = false;
        }

        let lost_focus = output.response.lost_focus();
        let enter = ui.input(|input| input.key_pressed(Key::Enter));
        let escape = ui.input(|input| input.key_pressed(Key::Escape));

        if lost_focus && escape {
            return ValueEntryEvent::Canceled;
        }

        if lost_focus {
            if let Some(value) = self.units.parse_mono(&self.text) {
                return ValueEntryEvent::Committed(value);
            }

            if enter {
                output.response.request_focus();
            } else {
                return ValueEntryEvent::Canceled;
            }
        }

        ValueEntryEvent::Editing
    }
}

impl Default for ValueEntry {
    fn default() -> Self {
        Self::new(Units::Normalized, 0.0)
    }
}

pub(crate) enum ValueEntryEvent {
    Editing,
    Canceled,
    Committed(Smp),
}

/// Above `center`, or below when that would leave the screen. Side edges pull it in.
pub(crate) fn anchor(bounds: Rect, center: Pos2, gap: f32) -> (Align2, Pos2) {
    let above = center.y - gap >= bounds.top() + 20.0;
    let y = if above {
        center.y - gap
    } else {
        center.y + gap
    };
    let (pivot, x) = if center.x - REACH < bounds.left() {
        let pivot = if above {
            Align2::LEFT_BOTTOM
        } else {
            Align2::LEFT_TOP
        };

        (pivot, bounds.left())
    } else if center.x + REACH > bounds.right() {
        let pivot = if above {
            Align2::RIGHT_BOTTOM
        } else {
            Align2::RIGHT_TOP
        };

        (pivot, bounds.right())
    } else {
        let pivot = if above {
            Align2::CENTER_BOTTOM
        } else {
            Align2::CENTER_TOP
        };

        (pivot, center.x)
    };

    (pivot, pos2(x, y))
}
