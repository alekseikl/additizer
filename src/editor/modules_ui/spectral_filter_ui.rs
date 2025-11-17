use egui_baseview::egui::{Checkbox, Grid, Ui};

use crate::{
    editor::{
        ModuleUI, direct_input::DirectInput, modulation_input::ModulationInput,
        utils::confirm_module_removal,
    },
    synth_engine::{ModuleId, ModuleInput, SpectralFilter, SynthEngine},
};

pub struct SpectralFilterUI {
    module_id: ModuleId,
    remove_confirmation: bool,
}

impl SpectralFilterUI {
    pub fn new(module_id: ModuleId) -> Self {
        Self {
            module_id,
            remove_confirmation: false,
        }
    }

    fn filter<'a>(&mut self, synth: &'a mut SynthEngine) -> &'a mut SpectralFilter {
        SpectralFilter::downcast_mut_unwrap(synth.get_module_mut(self.module_id))
    }
}

impl ModuleUI for SpectralFilterUI {
    fn module_id(&self) -> ModuleId {
        self.module_id
    }

    fn ui(&mut self, synth: &mut SynthEngine, ui: &mut Ui) {
        let mut ui_data = self.filter(synth).get_ui();

        ui.heading("Spectral filter");
        ui.add_space(20.0);

        Grid::new("sf_grid")
            .num_columns(2)
            .spacing([40.0, 24.0])
            .striped(true)
            .show(ui, |ui| {
                ui.label("Input");
                ui.add(DirectInput::new(
                    synth,
                    ModuleInput::spectrum(self.module_id),
                ));
                ui.end_row();

                ui.label("Cutoff");
                if ui
                    .add(ModulationInput::new(
                        &mut ui_data.cutoff,
                        synth,
                        ModuleInput::cutoff(self.module_id),
                    ))
                    .changed()
                {
                    self.filter(synth).set_cutoff(ui_data.cutoff);
                }
                ui.end_row();

                ui.label("Q");
                if ui
                    .add(ModulationInput::new(
                        &mut ui_data.q,
                        synth,
                        ModuleInput::q(self.module_id),
                    ))
                    .changed()
                {
                    self.filter(synth).set_q(ui_data.q);
                }
                ui.end_row();

                ui.label("Four pole");
                if ui
                    .add(Checkbox::without_text(&mut ui_data.four_pole))
                    .changed()
                {
                    self.filter(synth).set_four_pole(ui_data.four_pole);
                }
                ui.end_row();
            });

        ui.add_space(40.0);

        if confirm_module_removal(ui, &mut self.remove_confirmation) {
            synth.remove_module(self.module_id);
        }
    }
}
