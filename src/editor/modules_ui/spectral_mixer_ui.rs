use egui_baseview::egui::{ComboBox, Grid, Slider, Ui};

use crate::{
    editor::{
        ModuleUI, direct_input::DirectInput, modulation_input::ModulationInput,
        module_label::ModuleLabel, utils::confirm_module_removal,
    },
    synth_engine::{Input, MixType, ModuleId, SpectralMixer, SynthEngine, VolumeType},
};

impl MixType {
    fn label(&self) -> &'static str {
        match self {
            Self::Add => "+",
            Self::Subtract => "-",
            Self::Multiply => "*",
        }
    }
}

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

    fn mixer_module(module_id: ModuleId, synth: &mut SynthEngine) -> &mut SpectralMixer {
        SpectralMixer::downcast_mut_unwrap(synth.get_module_mut(module_id))
    }

    fn mixer<'a>(&self, synth: &'a mut SynthEngine) -> &'a mut SpectralMixer {
        Self::mixer_module(self.module_id, synth)
    }
}

impl ModuleUI for SpectralMixerUi {
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

                let module_id = self.module_id;

                for input_idx in 0..ui_data.num_inputs {
                    let vol_type = ui_data.input_params[input_idx].volume_type;
                    let (input, value) = match vol_type {
                        VolumeType::Db => (
                            Input::LevelMix(input_idx),
                            &mut ui_data.input_levels[input_idx],
                        ),
                        VolumeType::Gain => (
                            Input::GainMix(input_idx),
                            &mut ui_data.input_gains[input_idx],
                        ),
                    };

                    ui.label(format!("Input {}", input_idx + 1));
                    if ui
                        .add(ModulationInput::new(value, synth, input, module_id).before(
                            move |ui, synth| {
                                if input_idx > 0 {
                                    let mix_type = &mut ui_data.input_params[input_idx].mix_type;

                                    ComboBox::from_id_salt(format!("mix-type-{}", input_idx))
                                        .selected_text(mix_type.label())
                                        .width(0.0)
                                        .show_ui(ui, |ui| {
                                            const TYPE_OPTIONS: &[MixType] = &[
                                                MixType::Add,
                                                MixType::Subtract,
                                                MixType::Multiply,
                                            ];

                                            for mix_type_item in TYPE_OPTIONS {
                                                if ui
                                                    .selectable_value(
                                                        mix_type,
                                                        *mix_type_item,
                                                        mix_type_item.label(),
                                                    )
                                                    .clicked()
                                                {
                                                    Self::mixer_module(module_id, synth)
                                                        .set_mix_type(input_idx, *mix_type);
                                                }
                                            }
                                        });
                                }

                                ui.add(DirectInput::new(
                                    synth,
                                    Input::SpectrumMix(input_idx),
                                    module_id,
                                ));

                                let volume_type = &mut ui_data.input_params[input_idx].volume_type;

                                ComboBox::from_id_salt(format!("volume-type-{}", input_idx))
                                    .selected_text(volume_type.label())
                                    .width(0.0)
                                    .show_ui(ui, |ui| {
                                        const TYPE_OPTIONS: &[VolumeType] =
                                            &[VolumeType::Gain, VolumeType::Db];

                                        for vol_type_item in TYPE_OPTIONS {
                                            if ui
                                                .selectable_value(
                                                    volume_type,
                                                    *vol_type_item,
                                                    vol_type_item.label(),
                                                )
                                                .clicked()
                                            {
                                                Self::mixer_module(module_id, synth)
                                                    .set_volume_type(input_idx, *volume_type);
                                            }
                                        }
                                    });
                            },
                        ))
                        .changed()
                    {
                        match vol_type {
                            VolumeType::Db => {
                                self.mixer(synth)
                                    .set_input_level(input_idx, ui_data.input_levels[input_idx]);
                            }
                            VolumeType::Gain => {
                                self.mixer(synth)
                                    .set_input_gain(input_idx, ui_data.input_gains[input_idx]);
                            }
                        }
                    }
                    ui.end_row();
                }

                let (input, value) = match ui_data.output_volume_type {
                    VolumeType::Db => (Input::Level, &mut ui_data.output_level),
                    VolumeType::Gain => (Input::Gain, &mut ui_data.output_gain),
                };

                ui.label("Output");
                if ui
                    .add(
                        ModulationInput::new(value, synth, input, self.module_id).before(
                            move |ui, synth| {
                                ComboBox::from_id_salt("volume-type-output")
                                    .selected_text(ui_data.output_volume_type.label())
                                    .width(0.0)
                                    .show_ui(ui, |ui| {
                                        const TYPE_OPTIONS: &[VolumeType] =
                                            &[VolumeType::Gain, VolumeType::Db];

                                        for vol_type_item in TYPE_OPTIONS {
                                            if ui
                                                .selectable_value(
                                                    &mut ui_data.output_volume_type,
                                                    *vol_type_item,
                                                    vol_type_item.label(),
                                                )
                                                .clicked()
                                            {
                                                Self::mixer_module(module_id, synth)
                                                    .set_output_volume_type(
                                                        ui_data.output_volume_type,
                                                    );
                                            }
                                        }
                                    });
                            },
                        ),
                    )
                    .changed()
                {
                    match ui_data.output_volume_type {
                        VolumeType::Db => {
                            self.mixer(synth).set_output_level(ui_data.output_level);
                        }
                        VolumeType::Gain => {
                            self.mixer(synth).set_output_gain(ui_data.output_gain);
                        }
                    }
                }
                ui.end_row();
            });

        ui.add_space(40.0);

        if confirm_module_removal(ui, &mut self.remove_confirmation) {
            synth.remove_module(self.module_id);
        }
    }
}
