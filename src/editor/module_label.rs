use egui::{
    Color32, FontFamily, FontId, Id, Response, RichText, Stroke, TextEdit, Ui, Widget, ecolor::Hsva,
};

use crate::synth_engine::{ModuleId, ModuleType, ui_bridge::UiBridge};

use super::utils::hsva;

const REMOVE_ICON: &str = "❌";
const REMOVE_TINT: Color32 = Color32::from_rgb(0xe0, 0x6a, 0x6a);

const BG_COLOR: Hsva = hsva(0.115, 0.05, 0.0075, 1.0);
const BORDER_COLOR: Hsva = hsva(0.115, 0.05, 0.2, 1.0);

pub struct ModuleLabel<'a> {
    synth_bridge: &'a mut UiBridge,
    module_id: ModuleId,
    module_type: ModuleType,
}

impl<'a> ModuleLabel<'a> {
    pub fn new(id: ModuleId, module_type: ModuleType, synth_bridge: &'a mut UiBridge) -> Self {
        Self {
            synth_bridge,
            module_id: id,
            module_type,
        }
    }
}

impl Widget for ModuleLabel<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let Self {
            synth_bridge: bridge,
            module_id,
            module_type,
        } = self;

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;

            let state_id = Id::new(("module-label-edit", module_id));
            let input_id = state_id.with("text");
            let focused = ui.memory(|m| m.has_focus(input_id));

            let mut buffer = focused
                .then(|| ui.data_mut(|d| d.remove_temp(state_id)))
                .flatten()
                .unwrap_or_else(|| bridge.get_module_label(module_id));

            let input = ui
                .scope(|ui| {
                    // Always show the hover border, not only while hovered.
                    // let stroke = ui.visuals().widgets.hovered.bg_stroke;
                    ui.visuals_mut().widgets.inactive.bg_stroke =
                        Stroke::new(1.0, Color32::from(BORDER_COLOR));

                    let font = FontId::new(11.0, FontFamily::Name("Bold".into()));
                    ui.add(
                        TextEdit::singleline(&mut buffer)
                            .id(input_id)
                            .desired_width(200.0)
                            .hint_text(
                                RichText::new(module_type.default_label()).font(font.clone()),
                            )
                            .font(font)
                            .background_color(Color32::from(BG_COLOR)),
                    )
                })
                .inner;

            if input.changed() {
                bridge.set_module_label(module_id, buffer.trim().to_string());
            }

            if focused {
                ui.data_mut(|d| d.insert_temp(state_id, buffer));
            }

            if ui
                .button(RichText::new(REMOVE_ICON).color(REMOVE_TINT))
                .on_hover_text("Remove Module")
                .clicked()
            {
                bridge.remove_module(module_id);
            }
        })
        .response
    }
}
