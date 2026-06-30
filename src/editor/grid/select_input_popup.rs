use egui::containers::menu::menu_style;
use egui::{
    Align, Color32, CornerRadius, Frame, Id, Label, LayerId, Layout, Margin, Order, Popup,
    PopupKind, Pos2, Response, RichText, Sense, TextStyle, Ui, UiBuilder, vec2,
};

use crate::synth_engine::{
    InputId, ModuleId,
    ui_bridge::{LinkableInput, UiBridge},
};

const IO_DOT_SIZE: f32 = 8.0;
const MENU_INDENT: f32 = 8.0;
const MENU_CONTENT_PAD: f32 = 6.0;
const MIN_MENU_WIDTH: f32 = 100.0;
const LINK_ICON: &str = "\u{2194}";

enum MenuItemIcon {
    InputDot(Color32),
    Link,
}

pub struct SelectInputPopup {
    pub src: ModuleId,
    pub dst: ModuleId,
    pub pos: Pos2,
}

impl SelectInputPopup {
    /// Returns `true` if the link request should be cleared.
    pub fn show(&self, ui: &mut Ui, bridge: &mut UiBridge) -> bool {
        let inputs = bridge.get_linkable_inputs(self.src, self.dst);

        if inputs.is_empty() {
            return true;
        }

        let menu_id = Id::new(("wire-link-menu", self.dst, self.src));
        let layer_id = LayerId::new(Order::Foreground, menu_id);

        let Some(popup) = Popup::new(menu_id, ui.ctx().clone(), self.pos, layer_id)
            .kind(PopupKind::Menu)
            .layout(Layout::top_down_justified(Align::Min))
            .style(menu_style)
            .gap(0.0)
            .frame(Frame::menu(ui.style()).inner_margin(Margin::ZERO))
            .show(|ui| {
                ui.set_width(content_width(ui, &inputs).max(MIN_MENU_WIDTH));
                ui.spacing_mut().item_spacing.y = 0.0;

                let row_count: usize = inputs.iter().map(|input| 1 + input.modulations.len()).sum();
                let mut row = 0;

                for input in &inputs {
                    let input_id = InputId::new(input.input_type, self.dst);
                    let color = input.input_type.color();
                    let label = input.input_type.label();
                    let is_first = row == 0;
                    let is_last = row + 1 == row_count;

                    if menu_item(
                        ui,
                        &label,
                        MenuItemIcon::InputDot(color),
                        0.0,
                        is_first,
                        is_last,
                    )
                    .clicked()
                    {
                        bridge.create_link(self.src, input_id);
                        ui.close();
                    }
                    row += 1;

                    for modulation in &input.modulations {
                        let is_first = row == 0;
                        let is_last = row + 1 == row_count;

                        if menu_item(
                            ui,
                            &modulation.label,
                            MenuItemIcon::Link,
                            MENU_INDENT,
                            is_first,
                            is_last,
                        )
                        .clicked()
                        {
                            bridge.set_link_modulation(modulation.module_id, &input_id, self.src);
                            ui.close();
                        }
                        row += 1;
                    }
                }
            })
        else {
            return true;
        };

        popup.response.should_close()
    }
}

fn text_width(ui: &Ui, label: &str) -> f32 {
    ui.painter()
        .layout_no_wrap(
            label.to_owned(),
            TextStyle::Body.resolve(ui.style()),
            ui.visuals().text_color(),
        )
        .size()
        .x
}

fn icon_width(ui: &Ui, icon: MenuItemIcon) -> f32 {
    match icon {
        MenuItemIcon::InputDot(_) => IO_DOT_SIZE,
        MenuItemIcon::Link => text_width(ui, LINK_ICON),
    }
}

fn row_content_width(ui: &Ui, label: &str, indent: f32, icon: MenuItemIcon) -> f32 {
    2.0 * MENU_CONTENT_PAD
        + indent
        + icon_width(ui, icon)
        + ui.spacing().item_spacing.x
        + text_width(ui, label)
}

fn content_width(ui: &Ui, inputs: &[LinkableInput]) -> f32 {
    inputs
        .iter()
        .flat_map(|input| {
            let label = input.input_type.label();
            std::iter::once(row_content_width(
                ui,
                &label,
                0.0,
                MenuItemIcon::InputDot(input.input_type.color()),
            ))
            .chain(
                input
                    .modulations
                    .iter()
                    .map(|m| row_content_width(ui, &m.label, MENU_INDENT, MenuItemIcon::Link)),
            )
        })
        .fold(0.0, f32::max)
}

fn highlight_radius(ui: &Ui, is_first: bool, is_last: bool) -> CornerRadius {
    let radius = ui.visuals().menu_corner_radius;

    match (is_first, is_last) {
        (true, true) => radius,
        (true, false) => CornerRadius {
            nw: radius.nw - 1,
            ne: radius.ne - 1,
            sw: 0,
            se: 0,
        },
        (false, true) => CornerRadius {
            nw: 0,
            ne: 0,
            sw: radius.sw - 1,
            se: radius.se - 1,
        },
        (false, false) => CornerRadius::ZERO,
    }
}

fn menu_item(
    ui: &mut Ui,
    label: &str,
    icon: MenuItemIcon,
    indent: f32,
    is_first: bool,
    is_last: bool,
) -> Response {
    let row_height = ui.spacing().interact_size.y;
    let row_width = ui.min_rect().width();

    let (rect, response) = ui.allocate_exact_size(vec2(row_width, row_height), Sense::click());
    let visuals = ui.style().interact(&response);
    let text_color = visuals.text_color();

    if ui.is_rect_visible(rect) && visuals.weak_bg_fill != Color32::TRANSPARENT {
        ui.painter().rect_filled(
            rect,
            highlight_radius(ui, is_first, is_last),
            visuals.weak_bg_fill,
        );
    }

    ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
        ui.horizontal(|ui| {
            ui.add_space(MENU_CONTENT_PAD + indent);

            match icon {
                MenuItemIcon::InputDot(color) => {
                    let (dot_rect, _) =
                        ui.allocate_exact_size(vec2(IO_DOT_SIZE, row_height), Sense::empty());
                    ui.painter()
                        .circle_filled(dot_rect.center(), IO_DOT_SIZE * 0.5, color);
                }
                MenuItemIcon::Link => {
                    ui.add(Label::new(LINK_ICON).selectable(false));
                }
            }

            ui.add(Label::new(RichText::new(label).color(text_color)).selectable(false));
        });
    });

    response
}
