use egui::{Checkbox, ComboBox, Grid, Ui};

use crate::{
    editor::{
        ModuleUi, direct_input::DirectInput, modulation_input::ModulationInput,
        module_label::ModuleLabel, utils::confirm_module_removal,
    },
    synth_engine::{
        Input, ModuleId, SpectralFilterType, spectral_filter, ui_bridge::UiBridge,
    },
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
    remove_confirmation: bool,
    label_state: Option<String>,
    filter_bridge: spectral_filter::UiBridge,
}

impl SpectralFilterUI {
    pub fn new(module_id: ModuleId, synth_bridge: &mut UiBridge) -> Option<Self> {
        let filter_bridge =
            spectral_filter::UiBridge::create(module_id, synth_bridge.synth().clone())?;

        Some(Self {
            remove_confirmation: false,
            label_state: None,
            filter_bridge,
        })
    }
}

impl ModuleUi for SpectralFilterUI {
    fn module_id(&self) -> Option<ModuleId> {
        Some(self.filter_bridge.module_id())
    }

    fn ui(&mut self, bridge: &mut UiBridge, ui: &mut Ui) {
        let module_id = self.filter_bridge.module_id();
        let mut config = self.filter_bridge.config().clone();

        ui.add(ModuleLabel::new(
            &mut self.label_state,
            bridge,
            module_id,
        ));

        ui.add_space(20.0);

        Grid::new("sf_grid")
            .num_columns(2)
            .spacing([40.0, 24.0])
            .striped(true)
            .show(ui, |ui| {
                ui.label("Input");
                ui.add(DirectInput::new(bridge, Input::Spectrum, module_id));
                ui.end_row();

                ui.label("Type");
                ComboBox::from_id_salt("spectral-filter-type")
                    .selected_text(config.filter_type.label())
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
                                    &mut config.filter_type,
                                    *filter_type,
                                    filter_type.label(),
                                )
                                .clicked()
                            {
                                self.filter_bridge.set_filter_type(*filter_type);
                            }
                        }
                    });
                ui.end_row();

                ui.label("Cutoff");
                if ui
                    .add(ModulationInput::new(
                        &mut config.cutoff,
                        bridge,
                        Input::Cutoff,
                        module_id,
                    ))
                    .changed()
                {
                    self.filter_bridge.set_param(Input::Cutoff, config.cutoff);
                }
                ui.end_row();

                ui.label("Q");
                if ui
                    .add(ModulationInput::new(
                        &mut config.q,
                        bridge,
                        Input::Q,
                        module_id,
                    ))
                    .changed()
                {
                    self.filter_bridge.set_param(Input::Q, config.q);
                }
                ui.end_row();

                ui.label("Drive");
                if ui
                    .add(ModulationInput::new(
                        &mut config.drive,
                        bridge,
                        Input::Drive,
                        module_id,
                    ))
                    .changed()
                {
                    self.filter_bridge.set_param(Input::Drive, config.drive);
                }
                ui.end_row();

                ui.label("Fourth order");
                if ui
                    .add(Checkbox::without_text(&mut config.fourth_order))
                    .changed()
                {
                    self.filter_bridge.set_fourth_order(config.fourth_order);
                }
                ui.end_row();

                ui.label("Linear phase");
                if ui
                    .add(Checkbox::without_text(&mut config.linear_phase))
                    .changed()
                {
                    self.filter_bridge.set_linear_phase(config.linear_phase);
                }
                ui.end_row();
            });

        ui.add_space(40.0);

        if confirm_module_removal(ui, &mut self.remove_confirmation) {
            bridge.remove_module(module_id);
        }
    }
}
