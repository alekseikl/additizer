use egui::{ComboBox, Response, Ui, Widget};

use crate::{
    editor::SynthEngineHandle,
    synth_engine::{Input, ModuleId, ModuleInput},
};

pub struct DirectInput {
    synth_engine: SynthEngineHandle,
    input: ModuleInput,
}

impl DirectInput {
    pub fn new(synth_engine: SynthEngineHandle, input: Input, module_id: ModuleId) -> Self {
        Self {
            synth_engine,
            input: ModuleInput::new(input, module_id),
        }
    }

    fn select_output(&mut self, output: ModuleId) {
        self.synth_engine
            .lock()
            .set_direct_link(output, self.input)
            .unwrap_or_else(|_| println!("Failed to select output"))
    }
}

impl Widget for DirectInput {
    fn ui(mut self, ui: &mut Ui) -> Response {
        let (available, connected) = {
            let synth = self.synth_engine.lock();
            (
                synth.get_available_input_sources(self.input),
                synth.get_connected_input_sources(self.input),
            )
        };
        let mut selected = connected.first().map(|src| src.src);

        ComboBox::from_id_salt(format!("direct-input-{:?}", self.input.input_type))
            .selected_text(
                connected
                    .first()
                    .map(|src| src.label.as_str())
                    .unwrap_or("Select Source"),
            )
            .show_ui(ui, |ui| {
                for src in &available {
                    if ui
                        .selectable_value(&mut selected, Some(src.src), &src.label)
                        .clicked()
                    {
                        self.select_output(src.src);
                    }
                }
            })
            .response
    }
}
