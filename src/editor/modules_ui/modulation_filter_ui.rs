use egui_baseview::egui::{Grid, Slider, Ui};

use crate::{
    editor::{
        ModuleUI, direct_input::DirectInput, module_label::ModuleLabel,
        utils::confirm_module_removal,
    },
    synth_engine::{ModulationFilter, ModuleId, ModuleInput, SynthEngine},
};

pub struct ModulationFilterUI {
    module_id: ModuleId,
    remove_confirmation: bool,
    label_state: Option<String>,
}

impl ModulationFilterUI {
    pub fn new(module_id: ModuleId) -> Self {
        Self {
            module_id,
            remove_confirmation: false,
            label_state: None,
        }
    }

    fn filter<'a>(&mut self, synth: &'a mut SynthEngine) -> &'a mut ModulationFilter {
        ModulationFilter::downcast_mut_unwrap(synth.get_module_mut(self.module_id))
    }
}

impl ModuleUI for ModulationFilterUI {
    fn module_id(&self) -> ModuleId {
        self.module_id
    }

    fn ui(&mut self, synth: &mut SynthEngine, ui: &mut Ui) {
        let mut ui_data = self.filter(synth).get_ui();

        ui.add(ModuleLabel::new(
            &ui_data.label,
            &mut self.label_state,
            synth.get_module_mut(self.module_id).unwrap(),
        ));

        ui.add_space(20.0);

        Grid::new("mfl_grid")
            .num_columns(2)
            .spacing([40.0, 24.0])
            .striped(true)
            .show(ui, |ui| {
                ui.label("Inputs");
                ui.add(DirectInput::new(synth, ModuleInput::audio(self.module_id)));
                ui.end_row();

                ui.label("Cutoff Frequency");
                ui.spacing_mut().slider_width = 200.0;

                if ui
                    .add(
                        Slider::new(&mut ui_data.cutoff_frequency, 50.0..=2_500.0)
                            .logarithmic(true),
                    )
                    .changed()
                {
                    self.filter(synth)
                        .set_cutoff_frequency(ui_data.cutoff_frequency);
                }
                ui.end_row();
            });

        ui.add_space(40.0);

        if confirm_module_removal(ui, &mut self.remove_confirmation) {
            synth.remove_module(self.module_id);
        }
    }
}
