use egui::{self, Button, Id, Modal, Response, Sides, TextEdit, Ui, Widget};

use crate::{
    editor::SynthEngineHandle,
    synth_engine::ModuleId,
};

pub struct ModuleLabel<'a> {
    label: &'a str,
    state: &'a mut Option<String>,
    synth_engine: &'a SynthEngineHandle,
    module_id: ModuleId,
}

impl<'a> ModuleLabel<'a> {
    pub fn new(
        label: &'a str,
        state: &'a mut Option<String>,
        synth_engine: &'a SynthEngineHandle,
        module_id: ModuleId,
    ) -> Self {
        Self {
            label,
            state,
            synth_engine,
            module_id,
        }
    }
}

impl Widget for ModuleLabel<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let result = ui
            .horizontal(|ui| {
                ui.heading(self.label);
                if ui.button("✏").clicked() {
                    *self.state = Some(self.label.to_string());
                }
            })
            .response;

        if let Some(label) = self.state {
            let trimmed = label.trim().to_string();

            let modal = Modal::new(Id::new("edit-label-modal")).show(ui.ctx(), |ui| {
                ui.set_width(280.0);
                ui.heading("Update Label");
                ui.add_space(16.0);
                ui.add(TextEdit::singleline(label)).request_focus();
                ui.add_space(32.0);

                Sides::new().show(
                    ui,
                    |_ui| {},
                    |ui| {
                        let save_clicked = ui
                            .add_enabled(!trimmed.is_empty(), Button::new("Save"))
                            .clicked();

                        if (save_clicked || ui.input(|i| i.key_pressed(egui::Key::Enter)))
                            && !trimmed.is_empty()
                        {
                            self.synth_engine
                                .lock()
                                .get_module_mut(self.module_id)
                                .unwrap()
                                .set_label(trimmed);
                            ui.close();
                        }

                        if ui.button("Discard").clicked() {
                            ui.close();
                        }
                    },
                );
            });

            if modal.should_close() {
                *self.state = None;
            }
        }

        result
    }
}
