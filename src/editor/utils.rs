use egui_baseview::egui::{Id, Modal, Sides, Ui};

pub fn confirm_module_removal(ui: &mut Ui, show_modal: &mut bool) -> bool {
    let mut remove = false;

    if ui.button("Remove Module").clicked() {
        *show_modal = true;
    }

    if *show_modal {
        let modal = Modal::new(Id::new("remove-mod-modal")).show(ui.ctx(), |ui| {
            ui.set_width(220.0);
            ui.heading("Confirm module remove?");
            ui.add_space(32.0);

            Sides::new().show(
                ui,
                |_ui| {},
                |ui| {
                    if ui.button("Confirm").clicked() {
                        remove = true;
                        ui.close();
                    }

                    if ui.button("Cancel").clicked() {
                        ui.close();
                    }
                },
            );
        });

        if modal.should_close() {
            *show_modal = false;
        }
    }

    remove
}
