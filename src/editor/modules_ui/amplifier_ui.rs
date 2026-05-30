use egui::{Grid, Ui};

use crate::{
    editor::{
        ModuleUi, SynthEngineHandle, modulation_input::ModulationInput, module_label::ModuleLabel,
        multi_input::MultiInput, utils::confirm_module_removal,
    },
    synth_engine::{Amplifier, Input, ModuleId, SynthEngine},
};

pub struct AmplifierUI {
    module_id: ModuleId,
    remove_confirmation: bool,
    module_label: Option<String>,
}

impl AmplifierUI {
    pub fn new(module_id: ModuleId) -> Self {
        Self {
            module_id,
            remove_confirmation: false,
            module_label: None,
        }
    }

    fn amp<'a>(&mut self, synth: &'a mut SynthEngine) -> &'a mut Amplifier {
        Amplifier::downcast_mut_unwrap(synth.get_module_mut(self.module_id))
    }
}

impl ModuleUi for AmplifierUI {
    fn module_id(&self) -> Option<ModuleId> {
        Some(self.module_id)
    }

    fn ui(&mut self, synth: &SynthEngineHandle, ui: &mut Ui) {
        let mut ui_data = self.amp(&mut synth.lock()).get_ui();

        {
            let mut s = synth.lock();
            ui.add(ModuleLabel::new(
                &ui_data.label,
                &mut self.module_label,
                s.get_module_mut(self.module_id).unwrap(),
            ));
        }

        ui.add_space(20.0);

        Grid::new("amp_grid")
            .num_columns(2)
            .spacing([40.0, 24.0])
            .striped(true)
            .show(ui, |ui| {
                ui.label("Inputs");
                ui.add(MultiInput::new(synth.clone(), Input::Audio, self.module_id));
                ui.end_row();

                ui.label("Gain");
                if ui
                    .add(
                        ModulationInput::new(
                            &mut ui_data.gain,
                            synth.clone(),
                            Input::Gain,
                            self.module_id,
                        )
                        .modulation_default(1.0),
                    )
                    .changed()
                {
                    self.amp(&mut synth.lock()).set_gain(ui_data.gain);
                }
                ui.end_row();
            });

        ui.add_space(40.0);

        if confirm_module_removal(ui, &mut self.remove_confirmation) {
            synth.lock().remove_module(self.module_id);
        }
    }
}
