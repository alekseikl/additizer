use egui_baseview::egui::{ComboBox, Grid, Ui};

use crate::{
    editor::{
        ModuleUI, direct_input::DirectInput, modulation_input::ModulationInput,
        module_label::ModuleLabel, utils::confirm_module_removal,
    },
    synth_engine::{Input, ModuleId, ShaperType, SynthEngine, WaveShaper},
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
        WaveShaper::downcast_mut_unwrap(synth.get_module_mut(self.module_id))
    }
}

impl ModuleUI for WaveShaperUi {
    fn module_id(&self) -> ModuleId {
        self.module_id
    }

    fn ui(&mut self, synth: &mut SynthEngine, ui: &mut Ui) {
        let mut ui_data = self.shaper(synth).get_ui();

        ui.add(ModuleLabel::new(
            &ui_data.label,
            &mut self.label_state,
            synth.get_module_mut(self.module_id).unwrap(),
        ));

        ui.add_space(20.0);

        Grid::new("waveshaper_grid")
            .num_columns(2)
            .spacing([40.0, 24.0])
            .striped(true)
            .show(ui, |ui| {
                ui.label("Input");
                ui.add(DirectInput::new(synth, Input::Audio, self.module_id));
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
                                self.shaper(synth).set_shaper_type(*shaper_type);
                            }
                        }
                    });
                ui.end_row();

                ui.label("Distortion");
                if ui
                    .add(ModulationInput::new(
                        &mut ui_data.distortion,
                        synth,
                        Input::Distortion,
                        self.module_id,
                    ))
                    .changed()
                {
                    self.shaper(synth).set_distortion(ui_data.distortion);
                }
                ui.end_row();

                ui.label("Clipping level");
                if ui
                    .add(ModulationInput::new(
                        &mut ui_data.clipping_level,
                        synth,
                        Input::ClippingLevel,
                        self.module_id,
                    ))
                    .changed()
                {
                    self.shaper(synth)
                        .set_clipping_level(ui_data.clipping_level);
                }
                ui.end_row();
            });

        ui.add_space(40.0);

        if confirm_module_removal(ui, &mut self.remove_confirmation) {
            synth.remove_module(self.module_id);
        }
    }
}
