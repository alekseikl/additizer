use egui_baseview::egui::{Grid, Ui};

use crate::{
    editor::{
        ModuleUI, modulation_input::ModulationInput, module_label::ModuleLabel,
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

impl ModuleUI for AmplifierUI {
    fn module_id(&self) -> ModuleId {
        self.module_id
    }

    fn ui(&mut self, synth: &mut SynthEngine, ui: &mut Ui) {
        let mut ui_data = self.amp(synth).get_ui();
        // let mut kill_time_ms = ui_data.voice_kill_time * 1000.0;

        ui.add(ModuleLabel::new(
            &ui_data.label,
            &mut self.module_label,
            synth.get_module_mut(self.module_id).unwrap(),
        ));

        ui.add_space(20.0);

        Grid::new("amp_grid")
            .num_columns(2)
            .spacing([40.0, 24.0])
            .striped(true)
            .show(ui, |ui| {
                ui.label("Inputs");
                ui.add(MultiInput::new(synth, Input::Audio, self.module_id));
                ui.end_row();

                ui.label("Level");
                if ui
                    .add(
                        ModulationInput::new(
                            &mut ui_data.level,
                            synth,
                            Input::Level,
                            self.module_id,
                        )
                        .modulation_default(1.0),
                    )
                    .changed()
                {
                    self.amp(synth).set_level(ui_data.level);
                }
                ui.end_row();
            });

        ui.add_space(40.0);

        if confirm_module_removal(ui, &mut self.remove_confirmation) {
            synth.remove_module(self.module_id);
        }
    }
}
