use std::collections::HashSet;

use egui::{ComboBox, Response, Ui, vec2};

use crate::synth_engine::{Input, ModuleId, ModuleInput, StereoSample, ui_bridge::UiBridge};

pub struct MultiInput {
    input: ModuleInput,
}

impl MultiInput {
    pub fn new(input: Input, module_id: ModuleId) -> Self {
        Self {
            input: ModuleInput::new(input, module_id),
        }
    }

    pub fn show(self, ui: &mut Ui, bridge: &mut UiBridge) -> Response {
        let available = bridge.get_available_input_sources(self.input);
        let connected = bridge.get_connected_input_sources(self.input);
        let connected_ids: HashSet<_> = HashSet::from_iter(connected.iter().map(|src| src.src));
        let filtered: Vec<_> = available
            .iter()
            .filter(|src| !connected_ids.contains(&src.src))
            .collect();

        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing = vec2(8.0, 8.0);

            for src in &connected {
                ui.horizontal(|ui| {
                    ui.label(&src.label);

                    if ui.button("❌").clicked() {
                        bridge.remove_link(src.src, self.input);
                    }
                });
            }

            if !filtered.is_empty() {
                ComboBox::from_id_salt("multi-input")
                    .selected_text("Add Source")
                    .show_ui(ui, |ui| {
                        for src in &filtered {
                            if ui.selectable_label(false, &src.label).clicked() {
                                bridge.add_link(src.src, self.input, StereoSample::ONE);
                            }
                        }
                    });
            }
        })
        .response
    }
}
