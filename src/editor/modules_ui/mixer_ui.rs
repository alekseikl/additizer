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
        let mixer_bridge = mixer::UiBridge::create(module_id, synth_bridge.engine().clone())?;

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
        let mut config = self.mixer_bridge.config().clone();
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
                    .add(Slider::new(&mut config.num_inputs, 1..=Mixer::MAX_INPUTS))
                    .changed()
                {
                    self.mixer_bridge.set_num_inputs(config.num_inputs);
                }
                ui.end_row();

                for input_idx in 0..config.num_inputs {
                    let input_volume_type_change = Rc::clone(&input_volume_type_change);
                    let i = input_idx as usize;
                    let mut volume_type = config.inputs[i].volume_type;
                    let mut value = match volume_type {
                        VolumeType::Db => config.inputs[i].level,
                        VolumeType::Gain => config.inputs[i].gain,
                    };
                    let input = match volume_type {
                        VolumeType::Db => Input::LevelMix(input_idx),
                        VolumeType::Gain => Input::GainMix(input_idx),
                    };

                    ui.label(format!("Input {}", input_idx + 1));
                    if ui
                        .add(
                            ModulationInput::new(&mut value, bridge, input, module_id).before(
                                move |ui, bridge| {
                                    ui.add(DirectInput::new(
                                        bridge,
                                        Input::AudioMix(input_idx),
                                        module_id,
                                    ));

                                    let volume_type_ref = &mut volume_type;

                                    ComboBox::from_id_salt(format!("volume-type-{}", input_idx))
                                        .selected_text(volume_type_ref.label())
                                        .width(0.0)
                                        .show_ui(ui, |ui| {
                                            const TYPE_OPTIONS: &[VolumeType] =
                                                &[VolumeType::Gain, VolumeType::Db];

                                            for vol_type_item in TYPE_OPTIONS {
                                                if ui
                                                    .selectable_value(
                                                        volume_type_ref,
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
                        match volume_type {
                            VolumeType::Db => {
                                self.mixer_bridge.set_param(Input::LevelMix(input_idx), value);
                            }
                            VolumeType::Gain => {
                                self.mixer_bridge.set_param(Input::GainMix(input_idx), value);
                            }
                        }
                    }
                    ui.end_row();

                    config.inputs[i].volume_type = volume_type;
                    match volume_type {
                        VolumeType::Db => config.inputs[i].level = value,
                        VolumeType::Gain => config.inputs[i].gain = value,
                    }
                }

                let mut output_volume_type = config.output_volume_type;
                let mut output_value = match output_volume_type {
                    VolumeType::Db => config.output_level,
                    VolumeType::Gain => config.output_gain,
                };
                let output_input = match output_volume_type {
                    VolumeType::Db => Input::Level,
                    VolumeType::Gain => Input::Gain,
                };

                ui.label("Output");
                let output_volume_type_change = Rc::clone(&output_volume_type_change);
                if ui
                    .add(
                        ModulationInput::new(&mut output_value, bridge, output_input, module_id)
                            .before(move |ui, _bridge| {
                                ComboBox::from_id_salt("volume-type-output")
                                    .selected_text(output_volume_type.label())
                                    .width(0.0)
                                    .show_ui(ui, |ui| {
                                        const TYPE_OPTIONS: &[VolumeType] =
                                            &[VolumeType::Gain, VolumeType::Db];

                                        for vol_type_item in TYPE_OPTIONS {
                                            if ui
                                                .selectable_value(
                                                    &mut output_volume_type,
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
                            }),
                    )
                    .changed()
                {
                    match output_volume_type {
                        VolumeType::Db => {
                            self.mixer_bridge.set_param(Input::Level, output_value);
                        }
                        VolumeType::Gain => {
                            self.mixer_bridge.set_param(Input::Gain, output_value);
                        }
                    }
                }
                ui.end_row();

                config.output_volume_type = output_volume_type;
                match output_volume_type {
                    VolumeType::Db => config.output_level = output_value,
                    VolumeType::Gain => config.output_gain = output_value,
                }
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
