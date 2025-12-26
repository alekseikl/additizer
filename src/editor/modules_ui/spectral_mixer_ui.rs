use egui_baseview::egui::{Grid, Slider, Ui};

use crate::{
    editor::{
        ModuleUI, modulation_input::ModulationInput, module_label::ModuleLabel,
        utils::confirm_module_removal,
    },
    synth_engine::{Input, ModuleId, SpectralMixer, SynthEngine},
};

pub struct SpectralMixerUi {
    module_id: ModuleId,
    remove_confirmation: bool,
    label_state: Option<String>,
}

impl SpectralMixerUi {
    pub fn new(module_id: ModuleId) -> Self {
        Self {
            module_id,
            remove_confirmation: false,
            label_state: None,
        }
    }

    fn mixer<'a>(&mut self, synth: &'a mut SynthEngine) -> &'a mut SpectralMixer {
        SpectralMixer::downcast_mut_unwrap(synth.get_module_mut(self.module_id))
    }
}

impl ModuleUI for SpectralMixerUi {
    fn module_id(&self) -> ModuleId {
        self.module_id
    }

    fn ui(&mut self, synth: &mut SynthEngine, ui: &mut Ui) {
        let mut ui_data = self.mixer(synth).get_ui();

        ui.add(ModuleLabel::new(
            &ui_data.label,
            &mut self.label_state,
            synth.get_module_mut(self.module_id).unwrap(),
        ));

        ui.add_space(20.0);

        Grid::new("spectral_mixer_grid")
            .num_columns(2)
            .spacing([40.0, 24.0])
            .striped(true)
            .show(ui, |ui| {
                ui.label("Inputs number");
                if ui
                    .add(Slider::new(
                        &mut ui_data.num_inputs,
                        1..=SpectralMixer::MAX_INPUTS,
                    ))
                    .changed()
                {
                    self.mixer(synth).set_num_inputs(ui_data.num_inputs);
                }
                ui.end_row();

                for input_idx in 0..ui_data.num_inputs {
                    ui.label(format!("Input {}", input_idx + 1));
                    if ui
                        .add(
                            ModulationInput::new(
                                &mut ui_data.input_volumes[input_idx],
                                synth,
                                Input::LevelDbMix(input_idx),
                                self.module_id,
                            )
                            .direct_input(Input::SpectrumMix(input_idx)),
                        )
                        .changed()
                    {
                        self.mixer(synth)
                            .set_input_volume(input_idx, ui_data.input_volumes[input_idx]);
                    }
                    ui.end_row();
                }

                ui.label("Output");
                if ui
                    .add(ModulationInput::new(
                        &mut ui_data.output_volume,
                        synth,
                        Input::LevelDb,
                        self.module_id,
                    ))
                    .changed()
                {
                    self.mixer(synth).set_output_volume(ui_data.output_volume);
                }
                ui.end_row();
            });

        ui.add_space(40.0);

        if confirm_module_removal(ui, &mut self.remove_confirmation) {
            synth.remove_module(self.module_id);
        }
    }
}
