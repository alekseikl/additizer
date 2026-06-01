use egui::{Grid, Ui};

use crate::{
    editor::{
        ModuleUi, direct_input::DirectInput, modulation_input::ModulationInput,
        module_label::ModuleLabel, utils::confirm_module_removal,
    },
    synth_engine::{Input, ModuleId, SpectralBlend, SynthEngine, ui_bridge::UiBridge},
};

pub struct SpectralBlendUi {
    module_id: ModuleId,
    remove_confirmation: bool,
    label_state: Option<String>,
}

impl SpectralBlendUi {
    pub fn new(module_id: ModuleId) -> Self {
        Self {
            module_id,
            remove_confirmation: false,
            label_state: None,
        }
    }

    fn blend<'a>(&mut self, synth: &'a mut SynthEngine) -> &'a mut SpectralBlend {
        synth.get_typed_module_mut(self.module_id).unwrap()
    }
}

impl ModuleUi for SpectralBlendUi {
    fn module_id(&self) -> Option<ModuleId> {
        Some(self.module_id)
    }

    fn ui(&mut self, bridge: &mut UiBridge, ui: &mut Ui) {
        let synth = bridge.synth();
        let mut ui_data = self.blend(&mut synth.lock()).get_ui();

        ui.add(ModuleLabel::new(
            &ui_data.label,
            &mut self.label_state,
            synth,
            self.module_id,
        ));

        ui.add_space(20.0);

        Grid::new("spectral_blend_grid")
            .num_columns(2)
            .spacing([40.0, 24.0])
            .striped(true)
            .show(ui, |ui| {
                ui.label("From");
                ui.add(DirectInput::new(synth.clone(), Input::Spectrum, self.module_id));
                ui.end_row();

                ui.label("To");
                ui.add(DirectInput::new(synth.clone(), Input::SpectrumTo, self.module_id));
                ui.end_row();

                ui.label("Blend");
                if ui
                    .add(ModulationInput::new(
                        &mut ui_data.blend,
                        synth.clone(),
                        Input::Blend,
                        self.module_id,
                    ))
                    .changed()
                {
                    self.blend(&mut synth.lock()).set_blend(ui_data.blend);
                }
                ui.end_row();
            });

        ui.add_space(40.0);

        if confirm_module_removal(ui, &mut self.remove_confirmation) {
            synth.lock().remove_module(self.module_id);
        }
    }
}
