use std::{cell::Cell, rc::Rc};

use egui::{ComboBox, Grid, Slider, Ui};

use crate::{
    editor::{
        ModuleUi, direct_input::DirectInput, modulation_input::ModulationInput,
        module_label::ModuleLabel, utils::confirm_module_removal,
    },
    synth_engine::{Input, Mixer, ModuleId, VolumeType, mixer, ui_bridge::UiBridge},
};

pub struct MixerUi {
    remove_confirmation: bool,
    label_state: Option<String>,
    mixer_bridge: mixer::UiBridge,
}

impl MixerUi {
    pub fn new(module_id: ModuleId, synth_bridge: &mut UiBridge) -> Option<Self> {
        let mixer_bridge = mixer::UiBridge::create(module_id, synth_bridge.synth().clone())?;

        Some(Self {
            remove_confirmation: false,
            label_state: None,
            mixer_bridge,
        })
    }
}

impl ModuleUi for MixerUi {
    fn module_id(&self) -> Option<ModuleId> {
        Some(self.mixer_bridge.module_id())
    }

    fn ui(&mut self, bridge: &mut UiBridge, ui: &mut Ui) {
        let module_id = self.mixer_bridge.module_id();
        let mut controls = self.mixer_bridge.controls().clone();
        let input_volume_type_change = Rc::new(Cell::new(None));
        let output_volume_type_change = Rc::new(Cell::new(None));

        ui.add(ModuleLabel::new(
            &mut self.label_state,
            bridge,
            module_id,
        ));

        ui.add_space(20.0);

        Grid::new("mixer_grid")
            .num_columns(2)
            .spacing([40.0, 24.0])
            .striped(true)
            .show(ui, |ui| {
                ui.label("Inputs number");
                if ui
                    .add(Slider::new(&mut controls.num_inputs, 1..=Mixer::MAX_INPUTS))
                    .changed()
                {
                    self.mixer_bridge.set_num_inputs(controls.num_inputs);
                }
                ui.end_row();

                for input_idx in 0..controls.num_inputs {
                    let input_volume_type_change = Rc::clone(&input_volume_type_change);
                    let i = input_idx as usize;
                    let vol_type = controls.input_volume_types[i];
                    let (input, value) = match vol_type {
                        VolumeType::Db => {
                            (Input::LevelMix(input_idx), &mut controls.input_levels[i])
                        }
                        VolumeType::Gain => {
                            (Input::GainMix(input_idx), &mut controls.input_gains[i])
                        }
                    };

                    ui.label(format!("Input {}", input_idx + 1));
                    if ui
                        .add(
                            ModulationInput::new(value, bridge, input, module_id).before(
                                move |ui, bridge| {
                                    ui.add(DirectInput::new(
                                        bridge,
                                        Input::AudioMix(input_idx),
                                        module_id,
                                    ));

                                    let volume_type = &mut controls.input_volume_types[i];

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
                                                    input_volume_type_change.set(Some((
                                                        input_idx,
                                                        *vol_type_item,
                                                    )));
                                                }
                                            }
                                        });
                                },
                            ),
                        )
                        .changed()
                    {
                        match vol_type {
                            VolumeType::Db => {
                                self.mixer_bridge
                                    .set_param(Input::LevelMix(input_idx), controls.input_levels[i]);
                            }
                            VolumeType::Gain => {
                                self.mixer_bridge
                                    .set_param(Input::GainMix(input_idx), controls.input_gains[i]);
                            }
                        }
                    }
                    ui.end_row();
                }

                let (input, value) = match controls.output_volume_type {
                    VolumeType::Db => (Input::Level, &mut controls.output_level),
                    VolumeType::Gain => (Input::Gain, &mut controls.output_gain),
                };

                ui.label("Output");
                let output_volume_type_change = Rc::clone(&output_volume_type_change);
                if ui
                    .add(
                        ModulationInput::new(value, bridge, input, module_id).before(
                            move |ui, _bridge| {
                                ComboBox::from_id_salt("volume-type-output")
                                    .selected_text(controls.output_volume_type.label())
                                    .width(0.0)
                                    .show_ui(ui, |ui| {
                                        const TYPE_OPTIONS: &[VolumeType] =
                                            &[VolumeType::Gain, VolumeType::Db];

                                        for vol_type_item in TYPE_OPTIONS {
                                            if ui
                                                .selectable_value(
                                                    &mut controls.output_volume_type,
                                                    *vol_type_item,
                                                    vol_type_item.label(),
                                                )
                                                .clicked()
                                            {
                                                output_volume_type_change
                                                    .set(Some(*vol_type_item));
                                            }
                                        }
                                    });
                            },
                        ),
                    )
                    .changed()
                {
                    match controls.output_volume_type {
                        VolumeType::Db => {
                            self.mixer_bridge
                                .set_param(Input::Level, controls.output_level);
                        }
                        VolumeType::Gain => {
                            self.mixer_bridge.set_param(Input::Gain, controls.output_gain);
                        }
                    }
                }
                ui.end_row();
            });

        if let Some((input_idx, volume_type)) = input_volume_type_change.take() {
            self.mixer_bridge.set_volume_type(input_idx, volume_type);
        }

        if let Some(volume_type) = output_volume_type_change.take() {
            self.mixer_bridge.set_output_volume_type(volume_type);
        }

        ui.add_space(40.0);

        if confirm_module_removal(ui, &mut self.remove_confirmation) {
            bridge.remove_module(module_id);
        }
    }
}
