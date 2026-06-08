use egui::{ComboBox, Grid, Ui};

use crate::{
    editor::{
        ModuleUi, direct_input::DirectInput, modulation_input::ModulationInput,
        module_label::ModuleLabel, utils::confirm_module_removal,
    },
    synth_engine::{
        Input, ModuleId, ShaperType, wave_shaper::WaveShaperUiBridge, ui_bridge::UiBridge,
    },
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
    module_id: ModuleId,
    remove_confirmation: bool,
    label_state: Option<String>,
}

impl WaveShaperUi {
    pub fn new(module_id: ModuleId) -> Self {
        Self {
            module_id,
            remove_confirmation: false,
            label_state: None,
        }
    }

    fn paint_ui(
        &mut self,
        bridge: &mut UiBridge,
        shaper_bridge: &mut WaveShaperUiBridge,
        ui: &mut Ui,
    ) {
        let module_id = self.module_id;
        let mut config = shaper_bridge.config().clone();

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
                    .selected_text(config.shaper_type.label())
                    .show_ui(ui, |ui| {
                        const TYPE_OPTIONS: &[ShaperType] =
                            &[ShaperType::HardClip, ShaperType::Sigmoid];

                        for shaper_type in TYPE_OPTIONS {
                            if ui
                                .selectable_value(
                                    &mut config.shaper_type,
                                    *shaper_type,
                                    shaper_type.label(),
                                )
                                .clicked()
                            {
                                shaper_bridge.set_shaper_type(*shaper_type);
                            }
                        }
                    });
                ui.end_row();

                ui.label("Distortion");
                if ui
                    .add(ModulationInput::new(
                        &mut config.distortion,
                        bridge,
                        Input::Distortion,
                        module_id,
                    ))
                    .changed()
                {
                    shaper_bridge.set_param(Input::Distortion, config.distortion);
                }
                ui.end_row();

                ui.label("Clipping level");
                if ui
                    .add(ModulationInput::new(
                        &mut config.clipping_level,
                        bridge,
                        Input::ClippingLevel,
                        module_id,
                    ))
                    .changed()
                {
                    shaper_bridge.set_param(Input::ClippingLevel, config.clipping_level);
                }
                ui.end_row();
            });

        ui.add_space(40.0);

        if confirm_module_removal(ui, &mut self.remove_confirmation) {
            bridge.remove_module(module_id);
        }
    }
}

impl ModuleUi for WaveShaperUi {
    fn module_id(&self) -> Option<ModuleId> {
        Some(self.module_id)
    }

    fn ui(&mut self, bridge: &mut UiBridge, ui: &mut Ui) {
        bridge.with_module_bridge(self.module_id, |bridge, shaper_bridge| {
            self.paint_ui(bridge, shaper_bridge, ui);
        });
    }
}
