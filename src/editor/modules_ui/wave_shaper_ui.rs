use egui::{ComboBox, Grid, Ui};

use crate::{
    editor::{
        ModuleUi, direct_input::DirectInput, modulation_input::ModulationInput,
        module_label::ModuleLabel, utils::confirm_module_removal,
    },
    synth_engine::{Input, ModuleId, ShaperType, SynthEngine, WaveShaper, ui_bridge::UiBridge},
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

    fn shaper<'a>(&mut self, synth: &'a mut SynthEngine) -> &'a mut WaveShaper {
        synth.get_typed_module_mut(self.module_id).unwrap()
    }
}

impl ModuleUi for WaveShaperUi {
    fn module_id(&self) -> Option<ModuleId> {
        Some(self.module_id)
    }

    fn ui(&mut self, bridge: &mut UiBridge, ui: &mut Ui) {
        let synth = bridge.synth().clone();
        let mut ui_data = self.shaper(&mut synth.lock()).get_ui();

        ui.add(ModuleLabel::new(
            &mut self.label_state,
            bridge,
            self.module_id,
        ));

        ui.add_space(20.0);

        Grid::new("waveshaper_grid")
            .num_columns(2)
            .spacing([40.0, 24.0])
            .striped(true)
            .show(ui, |ui| {
                ui.label("Input");
                ui.add(DirectInput::new(bridge, Input::Audio, self.module_id));
                ui.end_row();

                ui.label("Type");
                ComboBox::from_id_salt("waveshaper-type")
                    .selected_text(ui_data.shaper_type.label())
                    .show_ui(ui, |ui| {
                        const TYPE_OPTIONS: &[ShaperType] =
                            &[ShaperType::HardClip, ShaperType::Sigmoid];

                        for shaper_type in TYPE_OPTIONS {
                            if ui
                                .selectable_value(
                                    &mut ui_data.shaper_type,
                                    *shaper_type,
                                    shaper_type.label(),
                                )
                                .clicked()
                            {
                                self.shaper(&mut synth.lock()).set_shaper_type(*shaper_type);
                            }
                        }
                    });
                ui.end_row();

                ui.label("Distortion");
                if ui
                    .add(ModulationInput::new(
                        &mut ui_data.distortion,
                        bridge,
                        Input::Distortion,
                        self.module_id,
                    ))
                    .changed()
                {
                    self.shaper(&mut synth.lock())
                        .set_distortion(ui_data.distortion);
                }
                ui.end_row();

                ui.label("Clipping level");
                if ui
                    .add(ModulationInput::new(
                        &mut ui_data.clipping_level,
                        bridge,
                        Input::ClippingLevel,
                        self.module_id,
                    ))
                    .changed()
                {
                    self.shaper(&mut synth.lock())
                        .set_clipping_level(ui_data.clipping_level);
                }
                ui.end_row();
            });

        ui.add_space(40.0);

        if confirm_module_removal(ui, &mut self.remove_confirmation) {
            bridge.remove_module(self.module_id);
        }
    }
}
