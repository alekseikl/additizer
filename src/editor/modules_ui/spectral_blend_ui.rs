use egui_baseview::egui::{Grid, Ui};

use crate::{
    editor::{
        ModuleUI, modulation_input::ModulationInput, module_label::ModuleLabel,
        utils::confirm_module_removal,
    },
    synth_engine::{Input, ModuleId, SpectralBlend, StereoSample, SynthEngine},
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
        SpectralBlend::downcast_mut_unwrap(synth.get_module_mut(self.module_id))
    }
}

impl ModuleUI for SpectralBlendUi {
    fn module_id(&self) -> ModuleId {
        self.module_id
    }

    fn ui(&mut self, synth: &mut SynthEngine, ui: &mut Ui) {
        let mut ui_data = self.blend(synth).get_ui();
        let mut value_stub = StereoSample::ZERO;

        ui.add(ModuleLabel::new(
            &ui_data.label,
            &mut self.label_state,
            synth.get_module_mut(self.module_id).unwrap(),
        ));

        ui.add_space(20.0);

        Grid::new("spectral_blend_grid")
            .num_columns(2)
            .spacing([40.0, 24.0])
            .striped(true)
            .show(ui, |ui| {
                ui.label("From");
                ui.add(
                    ModulationInput::new(&mut value_stub, synth, Input::Spectrum, self.module_id)
                        .hide_value()
                        .modulation_default(1.0),
                );
                ui.end_row();

                ui.label("To");
                ui.add(
                    ModulationInput::new(&mut value_stub, synth, Input::SpectrumTo, self.module_id)
                        .hide_value()
                        .modulation_default(1.0),
                );
                ui.end_row();

                ui.label("Blend");
                if ui
                    .add(ModulationInput::new(
                        &mut ui_data.blend,
                        synth,
                        Input::Blend,
                        self.module_id,
                    ))
                    .changed()
                {
                    self.blend(synth).set_blend(ui_data.blend);
                }
                ui.end_row();
            });

        ui.add_space(40.0);

        if confirm_module_removal(ui, &mut self.remove_confirmation) {
            synth.remove_module(self.module_id);
        }
    }
}
