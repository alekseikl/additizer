use egui::{ComboBox, Grid, Ui};

use crate::{
    editor::{
        ModuleUi, direct_input::DirectInput, modulation_input::ModulationInput,
        module_label::ModuleLabel, utils::confirm_module_removal,
    },
    synth_engine::{Input, ModuleId, ShaperType, wave_shaper, ui_bridge::UiBridge},
};

impl ShaperType {
    fn label(&self) -> &'static str {
        match self {
            Self::HardClip => "Hard Clip",
            Self::Sigmoid => "Sigmoid",
        }
    }
}

pub struct WaveShaperUi {
    remove_confirmation: bool,
    label_state: Option<String>,
    shaper_bridge: wave_shaper::UiBridge,
}

impl WaveShaperUi {
    pub fn new(module_id: ModuleId, synth_bridge: &mut UiBridge) -> Option<Self> {
        let shaper_bridge =
            wave_shaper::UiBridge::create(module_id, synth_bridge.synth().clone())?;

        Some(Self {
            remove_confirmation: false,
            label_state: None,
            shaper_bridge,
        })
    }
}

impl ModuleUi for WaveShaperUi {
    fn module_id(&self) -> Option<ModuleId> {
        Some(self.shaper_bridge.module_id())
    }

    fn ui(&mut self, bridge: &mut UiBridge, ui: &mut Ui) {
        let module_id = self.shaper_bridge.module_id();
        let mut controls = self.shaper_bridge.controls().clone();

        ui.add(ModuleLabel::new(
            &mut self.label_state,
            bridge,
            module_id,
        ));

        ui.add_space(20.0);

        Grid::new("waveshaper_grid")
            .num_columns(2)
            .spacing([40.0, 24.0])
            .striped(true)
            .show(ui, |ui| {
                ui.label("Input");
                ui.add(DirectInput::new(bridge, Input::Audio, module_id));
                ui.end_row();

                ui.label("Type");
                ComboBox::from_id_salt("waveshaper-type")
                    .selected_text(controls.shaper_type.label())
                    .show_ui(ui, |ui| {
                        const TYPE_OPTIONS: &[ShaperType] =
                            &[ShaperType::HardClip, ShaperType::Sigmoid];

                        for shaper_type in TYPE_OPTIONS {
                            if ui
                                .selectable_value(
                                    &mut controls.shaper_type,
                                    *shaper_type,
                                    shaper_type.label(),
                                )
                                .clicked()
                            {
                                self.shaper_bridge.set_shaper_type(*shaper_type);
                            }
                        }
                    });
                ui.end_row();

                ui.label("Distortion");
                if ui
                    .add(ModulationInput::new(
                        &mut controls.distortion,
                        bridge,
                        Input::Distortion,
                        module_id,
                    ))
                    .changed()
                {
                    self.shaper_bridge
                        .set_param(Input::Distortion, controls.distortion);
                }
                ui.end_row();

                ui.label("Clipping level");
                if ui
                    .add(ModulationInput::new(
                        &mut controls.clipping_level,
                        bridge,
                        Input::ClippingLevel,
                        module_id,
                    ))
                    .changed()
                {
                    self.shaper_bridge
                        .set_param(Input::ClippingLevel, controls.clipping_level);
                }
                ui.end_row();
            });

        ui.add_space(40.0);

        if confirm_module_removal(ui, &mut self.remove_confirmation) {
            bridge.remove_module(module_id);
        }
    }
}
