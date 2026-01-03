use std::collections::HashSet;

use egui_baseview::egui::{ComboBox, Response, Ui, Widget, vec2};

use crate::synth_engine::{Input, ModuleId, ModuleInput, SynthEngine};

pub struct MultiInput<'a> {
    synth_engine: &'a mut SynthEngine,
    input: ModuleInput,
}

impl<'a> MultiInput<'a> {
    pub fn new(synth_engine: &'a mut SynthEngine, input: Input, module_id: ModuleId) -> Self {
        Self {
            synth_engine,
            input: ModuleInput::new(input, module_id),
        }
    }

    fn select_output(&mut self, output: ModuleId) {
        self.synth_engine
            .add_link(output, self.input, 1.0.into())
            .unwrap_or_else(|_| println!("Failed to select output"))
    }
}

impl Widget for MultiInput<'_> {
    fn ui(mut self, ui: &mut Ui) -> Response {
        let available = self.synth_engine.get_available_input_sources(self.input);
        let connected = self.synth_engine.get_connected_input_sources(self.input);
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
                        self.synth_engine.remove_link(&src.src, &self.input);
                    }
                });
            }

            if !filtered.is_empty() {
                ComboBox::from_id_salt("multi-input")
                    .selected_text("Add Source")
                    .show_ui(ui, |ui| {
                        for src in &filtered {
                            if ui.selectable_label(false, &src.label).clicked() {
                                self.select_output(src.src);
                            }
                        }
                    });
            }
        })
        .response
    }
}
