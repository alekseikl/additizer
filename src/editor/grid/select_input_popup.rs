use egui::containers::menu::menu_style;
use egui::{
    Align, Color32, CornerRadius, Frame, Id, Label, LayerId, Layout, Margin, Order, Popup,
    PopupKind, Pos2, Rect, Response, RichText, Sense, Ui, UiBuilder, vec2,
};

use crate::editor::grid::{GridEvent, WidgetCtx};
use crate::synth_engine::Input;
use crate::synth_engine::{InputId, ModuleId, ModuleType, ui_bridge::LinkableInput};

const IO_DOT_SIZE: f32 = 8.0;
const MENU_INDENT: i8 = 8;
const MENU_CONTENT_PAD: i8 = 6;
const MAX_ROW_WIDTH: f32 = 250.0;
const MULTIPLY_TINT: Color32 = Color32::from_rgb(0xff, 0xb0, 0x00);

enum MenuItemIcon {
    InputDot(Color32),
    Link,
}

pub enum ShowResult {
    MixedSelected(Input),
    Closed,
    KeepVisible,
}

pub struct SelectInputPopup {
    pub src: ModuleId,
    pub dst: ModuleId,
    pub pos: Pos2,
}

impl SelectInputPopup {
    pub fn show(&self, ui: &mut Ui, ctx: &mut WidgetCtx, module_type: ModuleType) -> ShowResult {
        let inputs = ctx.bridge.get_linkable_inputs(self.src, self.dst);

        if inputs.is_empty() {
            return ShowResult::Closed;
        }

        let menu_id = Id::new(("wire-link-menu", self.dst, self.src));
        let layer_id = LayerId::new(Order::Foreground, menu_id);
        let mut selected = None;

        let Some(popup) = Popup::new(menu_id, ui.ctx().clone(), self.pos, layer_id)
            .kind(PopupKind::Menu)
            .layout(Layout::top_down_justified(Align::Min))
            .style(menu_style)
            .gap(0.0)
            .frame(Frame::menu(ui.style()).inner_margin(Margin::ZERO))
            .show(|ui| {
                ui.spacing_mut().item_spacing.y = 0.0;

                let mut measuring = ui.new_child(UiBuilder::new().sizing_pass().invisible());
                self.rows(&mut measuring, ctx, module_type, &inputs, None);

                let row_width = measuring.min_rect().width();
                selected = self.rows(ui, ctx, module_type, &inputs, Some(row_width));
            })
        else {
            return ShowResult::Closed;
        };

        if let Some(input) = selected {
            return ShowResult::MixedSelected(input);
        }

        if popup.response.should_close() {
            ShowResult::Closed
        } else {
            ShowResult::KeepVisible
        }
    }

    fn rows(
        &self,
        ui: &mut Ui,
        ctx: &mut WidgetCtx,
        module_type: ModuleType,
        inputs: &[LinkableInput],
        row_width: Option<f32>,
    ) -> Option<Input> {
        let row_count: usize = inputs.iter().map(|input| 1 + input.modulations.len()).sum();
        let mut row = 0;

        for input in inputs {
            let input_id = InputId::new(input.input_type, self.dst);
            let color = input.input_type.color();
            let label = module_type.input_label(input.input_type);
            let is_first = row == 0;
            let is_last = row + 1 == row_count;

            if Self::menu_item(
                ui,
                &label,
                MenuItemIcon::InputDot(color),
                0,
                row_width,
                is_first,
                is_last,
            )
            .clicked()
            {
                ctx.bridge.create_link(self.src, input_id);
                ctx.events.push(GridEvent::Moved(self.dst));
                ui.close();

                if !input.is_direct {
                    return Some(input.input_type);
                }
            }
            row += 1;

            for modulation in &input.modulations {
                let is_first = row == 0;
                let is_last = row + 1 == row_count;

                if Self::menu_item(
                    ui,
                    &modulation.label,
                    MenuItemIcon::Link,
                    MENU_INDENT,
                    row_width,
                    is_first,
                    is_last,
                )
                .clicked()
                {
                    ctx.bridge
                        .set_link_modulation(modulation.module_id, &input_id, self.src);
                    ui.close();
                }
                row += 1;
            }
        }

        None
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

    /// `row_width` is `None` while measuring, in which case the row takes the width of its content.
    fn menu_item(
        ui: &mut Ui,
        label: &str,
        icon: MenuItemIcon,
        indent: i8,
        row_width: Option<f32>,
        is_first: bool,
        is_last: bool,
    ) -> Response {
        let row_height = ui.spacing().interact_size.y;

        let (rect, response) = ui.allocate_exact_size(
            vec2(row_width.unwrap_or_default(), row_height),
            Sense::click(),
        );
        let visuals = ui.style().interact(&response);
        let text_color = visuals.text_color();

        if ui.is_rect_visible(rect) && visuals.weak_bg_fill != Color32::TRANSPARENT {
            ui.painter().rect_filled(
                rect,
                Self::highlight_radius(ui, is_first, is_last),
                visuals.weak_bg_fill,
            );
        }

        let content_rect = match row_width {
            Some(_) => rect,
            None => Rect::from_min_size(rect.min, vec2(MAX_ROW_WIDTH, row_height)),
        };

        ui.scope_builder(
            UiBuilder::new()
                .max_rect(content_rect)
                .layout(Layout::left_to_right(Align::Center)),
            |ui| {
                Frame::NONE
                    .inner_margin(Margin {
                        left: MENU_CONTENT_PAD + indent,
                        right: MENU_CONTENT_PAD,
                        top: 0,
                        bottom: 0,
                    })
                    .show(ui, |ui| {
                        match icon {
                            MenuItemIcon::InputDot(color) => {
                                let (dot_rect, _) = ui.allocate_exact_size(
                                    vec2(IO_DOT_SIZE, row_height),
                                    Sense::empty(),
                                );
                                ui.painter().circle_filled(
                                    dot_rect.center(),
                                    IO_DOT_SIZE * 0.5,
                                    color,
                                );
                            }
                            MenuItemIcon::Link => {
                                ui.add(
                                    Label::new(RichText::new("×").color(MULTIPLY_TINT))
                                        .selectable(false),
                                );
                            }
                        }

                        ui.add(
                            Label::new(RichText::new(label).color(text_color))
                                .truncate()
                                .selectable(false),
                        );
                    });
            },
        );

        response
    }
}
