use egui_baseview::egui::{ComboBox, Response, Ui, Widget};

use crate::synth_engine::{Input, ModuleId, ModuleInput, SynthEngine};

pub struct DirectInput<'a> {
    synth_engine: &'a mut SynthEngine,
    input: ModuleInput,
}

impl<'a> DirectInput<'a> {
    pub fn new(synth_engine: &'a mut SynthEngine, input: Input, module_id: ModuleId) -> Self {
        Self {
            synth_engine,
            input: ModuleInput::new(input, module_id),
        }
    }

    fn select_output(&mut self, output: ModuleId) {
        self.synth_engine
            .set_direct_link(output, self.input)
            .unwrap_or_else(|_| println!("Failed to select output"))
    }
}

impl Widget for DirectInput<'_> {
    fn ui(mut self, ui: &mut Ui) -> Response {
        let available = self.synth_engine.get_available_input_sources(self.input);
        let connected = self.synth_engine.get_connected_input_sources(self.input);
        let mut selected = connected.first().map(|src| src.output);

        ComboBox::from_id_salt("direct-input")
            .selected_text(
                connected
                    .first()
                    .map(|src| src.label.as_str())
                    .unwrap_or("Select Source"),
            )
            .show_ui(ui, |ui| {
                for src in &available {
                    if ui
                        .selectable_value(&mut selected, Some(src.output), &src.label)
                        .clicked()
                    {
                        self.select_output(src.output);
                    }
                }
            })
            .response
    }
}
