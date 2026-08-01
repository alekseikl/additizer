use egui::containers::menu::menu_style;
use egui::{
    Align, FontFamily, FontId, Frame, Grid, Label, Layout, Margin, Popup, PopupCloseBehavior,
    PopupKind, Response, Sense, TextFormat, TextStyle, Ui,
    text::{LayoutJob, TextWrapping},
    vec2,
};

use crate::synth_engine::{Input, InputId, ModuleId, ModuleType, ui_bridge::UiBridge};

const MAX_LABEL_WIDTH: f32 = 200.0;
const IO_DOT_SIZE: f32 = 8.0;

pub struct LinkAmountPopup {
    pub src: ModuleId,
    pub module_id: ModuleId,
    pub module_type: ModuleType,
    pub input: Input,
}

impl LinkAmountPopup {
    /// Returns `true` on popup close
    pub fn show(&self, response: &Response, ui: &mut Ui, bridge: &mut UiBridge) -> bool {
        let input_id = InputId::new(self.input, self.module_id);
        let Some(src) = bridge
            .get_connected_input_sources(input_id)
            .into_iter()
            .find(|src| src.src == self.src)
        else {
            return true;
        };

        let Some(popup) = Popup::from_response(response)
            .kind(PopupKind::Popup)
            .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
            .layout(Layout::top_down(Align::Min))
            .style(menu_style)
            .gap(0.0)
            .frame(Frame::popup(ui.style()).inner_margin(Margin::same(8)))
            .show(|ui| {
                self.title_ui(ui, bridge);
                ui.add_space(8.0);

                Grid::new(("link-amount-row", self.module_id, self.input, self.src))
                    .num_columns(2)
                    .spacing([8.0, 4.0])
                    .min_col_width(0.0)
                    .striped(false)
                    .show(ui, |ui| {
                        ui.with_layout(Layout::top_down(Align::Max), |ui| {
                            ui.set_max_width(MAX_LABEL_WIDTH);
                            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                            ui.add(Label::new(&src.label).truncate());
                        });

                        let mut amount = src.amount;
                        let slider_response = ui.add(self.input.amount_slider(&mut amount));

                        if slider_response.changed() {
                            bridge.set_link_amount(self.src, input_id, amount);
                        }

                        ui.end_row();
                    });
            })
        else {
            return true;
        };

        popup.response.should_close()
    }

    fn title_ui(&self, ui: &mut Ui, bridge: &UiBridge) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = vec2(6.0, 0.0);

            let row_height = ui.spacing().interact_size.y;
            let (dot_rect, _) =
                ui.allocate_exact_size(vec2(IO_DOT_SIZE, row_height), Sense::empty());
            ui.painter()
                .circle_filled(dot_rect.center(), IO_DOT_SIZE * 0.5, self.input.color());

            let size = TextStyle::Body.resolve(ui.style()).size;
            let color = ui.visuals().text_color();
            let mut job = LayoutJob::default();
            job.append(
                &self.module_type.input_label(self.input),
                0.0,
                TextFormat {
                    font_id: FontId::new(size, FontFamily::Name("Bold".into())),
                    color,
                    ..Default::default()
                },
            );
            job.append(
                &format!(" ({})", bridge.display_module_label(self.module_id)),
                0.0,
                TextFormat {
                    font_id: FontId::new(size, FontFamily::Proportional),
                    color,
                    ..Default::default()
                },
            );
            job.wrap = TextWrapping::truncate_at_width(MAX_LABEL_WIDTH);
            ui.add(Label::new(job).truncate());
        });
    }
}
