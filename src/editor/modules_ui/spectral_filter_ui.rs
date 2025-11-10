use egui_baseview::egui::{Checkbox, Grid, Response, Ui, Widget};

use crate::{
    editor::modulation_input::ModulationInput,
    synth_engine::{ModuleId, ModuleInput, SpectralFilter, SynthEngine},
};

pub struct SpectralFilterUI<'a> {
    module_id: ModuleId,
    synth_engine: &'a mut SynthEngine,
}

impl<'a> SpectralFilterUI<'a> {
    pub fn new(module_id: ModuleId, synth_engine: &'a mut SynthEngine) -> Self {
        Self {
            module_id,
            synth_engine,
        }
    }

    fn filter(&mut self) -> &mut SpectralFilter {
        SpectralFilter::downcast_mut_unwrap(self.synth_engine.get_module_mut(self.module_id))
    }
}

impl Widget for SpectralFilterUI<'_> {
    fn ui(mut self, ui: &mut Ui) -> Response {
        let mut ui_data = self.filter().get_ui();

        ui.heading("Spectral filter");

        Grid::new("sf_grid")
            .num_columns(2)
            .spacing([40.0, 4.0])
            .striped(true)
            .show(ui, |ui| {
                ui.label("Cutoff");
                if ui
                    .add(ModulationInput::new(
                        &mut ui_data.cutoff,
                        self.synth_engine,
                        ModuleInput::cutoff(self.module_id),
                    ))
                    .changed()
                {
                    self.filter().set_cutoff(ui_data.cutoff);
                }
                ui.end_row();

                ui.label("Q");
                if ui
                    .add(ModulationInput::new(
                        &mut ui_data.q,
                        self.synth_engine,
                        ModuleInput::q(self.module_id),
                    ))
                    .changed()
                {
                    self.filter().set_q(ui_data.q);
                }
                ui.end_row();

                ui.label("Four pole");
                if ui
                    .add(Checkbox::without_text(&mut ui_data.four_pole))
                    .changed()
                {
                    self.filter().set_four_pole(ui_data.four_pole);
                }
                ui.end_row();
            })
            .response
    }
}
