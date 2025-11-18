use egui_baseview::egui::{self, Button, Id, Modal, Response, Sides, TextEdit, Ui, Widget};

use crate::synth_engine::SynthModule;

pub struct ModuleLabel<'a> {
    label: &'a str,
    state: &'a mut Option<String>,
    module: &'a mut dyn SynthModule,
}

impl<'a> ModuleLabel<'a> {
    pub fn new(
        label: &'a str,
        state: &'a mut Option<String>,
        module: &'a mut dyn SynthModule,
    ) -> Self {
        Self {
            label,
            state,
            module,
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
                            self.module.set_label(trimmed);
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
