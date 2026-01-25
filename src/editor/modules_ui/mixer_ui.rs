use egui_baseview::egui::{Grid, Slider, Ui};

use crate::{
    editor::{
        ModuleUI, direct_input::DirectInput, modulation_input::ModulationInput,
        module_label::ModuleLabel, utils::confirm_module_removal,
    },
    synth_engine::{Input, Mixer, ModuleId, SynthEngine},
};

pub struct MixerUi {
    module_id: ModuleId,
    remove_confirmation: bool,
    label_state: Option<String>,
}

impl MixerUi {
    pub fn new(module_id: ModuleId) -> Self {
        Self {
            module_id,
            remove_confirmation: false,
            label_state: None,
        }
    }

    fn mixer_module(module_id: ModuleId, synth: &mut SynthEngine) -> &mut Mixer {
        Mixer::downcast_mut_unwrap(synth.get_module_mut(module_id))
    }

    fn mixer<'a>(&self, synth: &'a mut SynthEngine) -> &'a mut Mixer {
        Self::mixer_module(self.module_id, synth)
    }
}

impl ModuleUI for MixerUi {
    fn module_id(&self) -> Option<ModuleId> {
        Some(self.module_id)
    }

    fn ui(&mut self, synth: &mut SynthEngine, ui: &mut Ui) {
        let mut ui_data = self.mixer(synth).get_ui();

        ui.add(ModuleLabel::new(
            &ui_data.label,
            &mut self.label_state,
            synth.get_module_mut(self.module_id).unwrap(),
        ));

        ui.add_space(20.0);

        Grid::new("mixer_grid")
            .num_columns(2)
            .spacing([40.0, 24.0])
            .striped(true)
            .show(ui, |ui| {
                ui.label("Inputs number");
                if ui
                    .add(Slider::new(&mut ui_data.num_inputs, 1..=Mixer::MAX_INPUTS))
                    .changed()
                {
                    self.mixer(synth).set_num_inputs(ui_data.num_inputs);
                }
                ui.end_row();

                let module_id = self.module_id;

                for input_idx in 0..ui_data.num_inputs {
                    ui.label(format!("Input {}", input_idx + 1));
                    if ui
                        .add(
                            ModulationInput::new(
                                &mut ui_data.input_levels[input_idx],
                                synth,
                                Input::LevelMix(input_idx),
                                module_id,
                            )
                            .before(move |ui, synth| {
                                ui.add(DirectInput::new(
                                    synth,
                                    Input::AudioMix(input_idx),
                                    module_id,
                                ));
                            }),
                        )
                        .changed()
                    {
                        self.mixer(synth)
                            .set_input_level(input_idx, ui_data.input_levels[input_idx]);
                    }
                    ui.end_row();
                }

                ui.label("Output");
                if ui
                    .add(ModulationInput::new(
                        &mut ui_data.output_level,
                        synth,
                        Input::Level,
                        self.module_id,
                    ))
                    .changed()
                {
                    self.mixer(synth).set_output_level(ui_data.output_level);
                }
                ui.end_row();
            });

        ui.add_space(40.0);

        if confirm_module_removal(ui, &mut self.remove_confirmation) {
            synth.remove_module(self.module_id);
        }
    }
}
