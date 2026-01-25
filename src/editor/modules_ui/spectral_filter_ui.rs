use egui_baseview::egui::{Checkbox, ComboBox, Grid, Ui};

use crate::{
    editor::{
        ModuleUI, direct_input::DirectInput, modulation_input::ModulationInput,
        module_label::ModuleLabel, utils::confirm_module_removal,
    },
    synth_engine::{Input, ModuleId, SpectralFilter, SpectralFilterType, SynthEngine},
};

impl SpectralFilterType {
    fn label(&self) -> &'static str {
        match self {
            Self::LowPass => "Lowpass",
            Self::HighPass => "Highpass",
            Self::BandPass => "Bandpass",
            Self::BandStop => "Bandstop",
            Self::Peaking => "Peaking",
        }
    }
}

pub struct SpectralFilterUI {
    module_id: ModuleId,
    remove_confirmation: bool,
    label_state: Option<String>,
}

impl SpectralFilterUI {
    pub fn new(module_id: ModuleId) -> Self {
        Self {
            module_id,
            remove_confirmation: false,
            label_state: None,
        }
    }

    fn filter<'a>(&mut self, synth: &'a mut SynthEngine) -> &'a mut SpectralFilter {
        SpectralFilter::downcast_mut_unwrap(synth.get_module_mut(self.module_id))
    }
}

impl ModuleUI for SpectralFilterUI {
    fn module_id(&self) -> Option<ModuleId> {
        Some(self.module_id)
    }

    fn ui(&mut self, synth: &mut SynthEngine, ui: &mut Ui) {
        let mut ui_data = self.filter(synth).get_ui();

        ui.add(ModuleLabel::new(
            &ui_data.label,
            &mut self.label_state,
            synth.get_module_mut(self.module_id).unwrap(),
        ));

        ui.add_space(20.0);

        Grid::new("sf_grid")
            .num_columns(2)
            .spacing([40.0, 24.0])
            .striped(true)
            .show(ui, |ui| {
                ui.label("Input");
                ui.add(DirectInput::new(synth, Input::Spectrum, self.module_id));
                ui.end_row();

                ui.label("Type");
                ComboBox::from_id_salt("spectral-filter-type")
                    .selected_text(ui_data.filter_type.label())
                    .show_ui(ui, |ui| {
                        const TYPE_OPTIONS: &[SpectralFilterType] = &[
                            SpectralFilterType::LowPass,
                            SpectralFilterType::HighPass,
                            SpectralFilterType::BandPass,
                            SpectralFilterType::BandStop,
                            SpectralFilterType::Peaking,
                        ];

                        for filter_type in TYPE_OPTIONS {
                            if ui
                                .selectable_value(
                                    &mut ui_data.filter_type,
                                    *filter_type,
                                    filter_type.label(),
                                )
                                .clicked()
                            {
                                self.filter(synth).set_filter_type(*filter_type);
                            }
                        }
                    });
                ui.end_row();

                ui.label("Cutoff");
                if ui
                    .add(ModulationInput::new(
                        &mut ui_data.cutoff,
                        synth,
                        Input::Cutoff,
                        self.module_id,
                    ))
                    .changed()
                {
                    self.filter(synth).set_cutoff(ui_data.cutoff);
                }
                ui.end_row();

                ui.label("Q");
                if ui
                    .add(ModulationInput::new(
                        &mut ui_data.q,
                        synth,
                        Input::Q,
                        self.module_id,
                    ))
                    .changed()
                {
                    self.filter(synth).set_q(ui_data.q);
                }
                ui.end_row();

                ui.label("Drive");
                if ui
                    .add(ModulationInput::new(
                        &mut ui_data.drive,
                        synth,
                        Input::Drive,
                        self.module_id,
                    ))
                    .changed()
                {
                    self.filter(synth).set_drive(ui_data.drive);
                }
                ui.end_row();

                ui.label("Fourth order");
                if ui
                    .add(Checkbox::without_text(&mut ui_data.fourth_order))
                    .changed()
                {
                    self.filter(synth).set_fourth_order(ui_data.fourth_order);
                }
                ui.end_row();
            });

        ui.add_space(40.0);

        if confirm_module_removal(ui, &mut self.remove_confirmation) {
            synth.remove_module(self.module_id);
        }
    }
}
