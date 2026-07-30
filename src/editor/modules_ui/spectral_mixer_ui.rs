use std::{cell::Cell, rc::Rc};

use egui::{ComboBox, Grid, Slider, Ui};

use crate::{
    editor::{module_label::ModuleLabel, stereo_input::StereoInput, ModuleUi},
    synth_engine::{
        spectral_mixer::SpectralMixerUiBridge,
        ui_bridge::{ModuleBridge, UiBridge},
        Input, MixType, ModuleId, SpectralMixer, VolumeType,
    },
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
    label_state: Option<String>,
}

impl SpectralMixerUi {
    pub fn new(module_id: ModuleId) -> Self {
        Self {
            module_id,
            label_state: None,
        }
    }

    fn paint_ui(
        &mut self,
        bridge: &mut UiBridge,
        mixer_bridge: &mut SpectralMixerUiBridge,
        ui: &mut Ui,
    ) {
        let module_id = self.module_id;
        let mut config = mixer_bridge.config().clone();
        let mix_type_change = Rc::new(Cell::new(None));
        let volume_type_change = Rc::new(Cell::new(None));
        let output_volume_type_change = Rc::new(Cell::new(None));

        ui.add(ModuleLabel::new(&mut self.label_state, bridge, module_id));

        ui.add_space(20.0);

        Grid::new("spectral_mixer_grid")
            .num_columns(2)
            .spacing([40.0, 24.0])
            .striped(true)
            .show(ui, |ui| {
                ui.label("Inputs number");
                if ui
                    .add(Slider::new(
                        &mut config.num_inputs,
                        1..=SpectralMixer::MAX_INPUTS,
                    ))
                    .changed()
                {
                    mixer_bridge.set_num_inputs(config.num_inputs);
                }
                ui.end_row();

                for input_idx in 0..config.num_inputs {
                    let mix_type_change = Rc::clone(&mix_type_change);
                    let volume_type_change = Rc::clone(&volume_type_change);
                    let i = input_idx as usize;
                    let mut mix_type = config.inputs[i].mix_type;
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
                    let response = ui
                        .horizontal(|ui| {
                            if input_idx > 0 {
                                ComboBox::from_id_salt(format!("mix-type-{}", input_idx))
                                    .selected_text(mix_type.label())
                                    .width(0.0)
                                    .show_ui(ui, |ui| {
                                        const TYPE_OPTIONS: &[MixType] =
                                            &[MixType::Add, MixType::Subtract, MixType::Multiply];

                                        for mix_type_item in TYPE_OPTIONS {
                                            if ui
                                                .selectable_value(
                                                    &mut mix_type,
                                                    *mix_type_item,
                                                    mix_type_item.label(),
                                                )
                                                .clicked()
                                            {
                                                mix_type_change
                                                    .set(Some((input_idx, *mix_type_item)));
                                            }
                                        }
                                    });
                            }

                            ComboBox::from_id_salt(format!("volume-type-{}", input_idx))
                                .selected_text(volume_type.label())
                                .width(0.0)
                                .show_ui(ui, |ui| {
                                    const TYPE_OPTIONS: &[VolumeType] =
                                        &[VolumeType::Gain, VolumeType::Db];

                                    for vol_type_item in TYPE_OPTIONS {
                                        if ui
                                            .selectable_value(
                                                &mut volume_type,
                                                *vol_type_item,
                                                vol_type_item.label(),
                                            )
                                            .clicked()
                                        {
                                            volume_type_change
                                                .set(Some((input_idx, *vol_type_item)));
                                        }
                                    }
                                });

                            ui.add(StereoInput::new(input, module_id, &mut value, bridge))
                        })
                        .inner;
                    if response.changed() {
                        match volume_type {
                            VolumeType::Db => {
                                mixer_bridge.set_param(Input::LevelMix(input_idx), value);
                            }
                            VolumeType::Gain => {
                                mixer_bridge.set_param(Input::GainMix(input_idx), value);
                            }
                        }
                    }
                    ui.end_row();

                    config.inputs[i].mix_type = mix_type;
                    config.inputs[i].volume_type = volume_type;
                    match volume_type {
                        VolumeType::Db => config.inputs[i].level = value,
                        VolumeType::Gain => config.inputs[i].gain = value,
                    }
                }

                let (input, value) = match config.output_volume_type {
                    VolumeType::Db => (Input::Level, &mut config.output_level),
                    VolumeType::Gain => (Input::Gain, &mut config.output_gain),
                };

                ui.label("Output");
                let output_volume_type_change = Rc::clone(&output_volume_type_change);
                let response = ui
                    .horizontal(|ui| {
                        ComboBox::from_id_salt("volume-type-output")
                            .selected_text(config.output_volume_type.label())
                            .width(0.0)
                            .show_ui(ui, |ui| {
                                const TYPE_OPTIONS: &[VolumeType] =
                                    &[VolumeType::Gain, VolumeType::Db];

                                for vol_type_item in TYPE_OPTIONS {
                                    if ui
                                        .selectable_value(
                                            &mut config.output_volume_type,
                                            *vol_type_item,
                                            vol_type_item.label(),
                                        )
                                        .clicked()
                                    {
                                        output_volume_type_change.set(Some(*vol_type_item));
                                    }
                                }
                            });

                        ui.add(StereoInput::new(input, module_id, value, bridge))
                    })
                    .inner;
                if response.changed() {
                    match config.output_volume_type {
                        VolumeType::Db => {
                            mixer_bridge.set_param(Input::Level, config.output_level);
                        }
                        VolumeType::Gain => {
                            mixer_bridge.set_param(Input::Gain, config.output_gain);
                        }
                    }
                }
                ui.end_row();
            });

        if let Some((input_idx, mix_type)) = mix_type_change.take() {
            mixer_bridge.set_mix_type(input_idx, mix_type);
        }

        if let Some((input_idx, volume_type)) = volume_type_change.take() {
            mixer_bridge.set_volume_type(input_idx, volume_type);
        }

        if let Some(volume_type) = output_volume_type_change.take() {
            mixer_bridge.set_output_volume_type(volume_type);
        }
    }
}

impl ModuleUi for SpectralMixerUi {
    fn module_id(&self) -> Option<ModuleId> {
        Some(self.module_id)
    }

    fn ui(&mut self, bridge: &mut UiBridge, ui: &mut Ui) {
        bridge.with_module_bridge(self.module_id, |bridge, module_bridge| {
            if let ModuleBridge::SpectralMixer(mixer_bridge) = module_bridge {
                self.paint_ui(bridge, mixer_bridge, ui);
            }
        });
    }
}
