use egui::{Grid, Ui};

use crate::{
    editor::{
        ModuleUi, modulation_input::ModulationInput, module_label::ModuleLabel,
        multi_input::MultiInput, utils::confirm_module_removal,
    },
    synth_engine::{Amplifier, Input, ModuleId, SynthEngine, ui_bridge::UiBridge},
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
        synth.get_typed_module_mut(self.module_id).unwrap()
    }
}

impl ModuleUi for AmplifierUI {
    fn module_id(&self) -> Option<ModuleId> {
        Some(self.module_id)
    }

    fn ui(&mut self, bridge: &mut UiBridge, ui: &mut Ui) {
        let synth = bridge.synth().clone();
        let mut ui_data = self.amp(&mut synth.lock()).get_ui();

        ui.add(ModuleLabel::new(
            &mut self.module_label,
            bridge,
            self.module_id,
        ));

        ui.add_space(20.0);

        Grid::new("amp_grid")
            .num_columns(2)
            .spacing([40.0, 24.0])
            .striped(true)
            .show(ui, |ui| {
                ui.label("Inputs");
                MultiInput::new(Input::Audio, self.module_id).show(ui, bridge);
                ui.end_row();

                ui.label("Gain");
                if ui
                    .add(
                        ModulationInput::new(
                            &mut ui_data.gain,
                            bridge,
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
            bridge.remove_module(self.module_id);
        }
    }
}
