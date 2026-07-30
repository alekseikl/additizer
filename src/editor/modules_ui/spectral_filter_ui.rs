use egui::{Align, Checkbox, ComboBox, Grid, Layout, Ui};

use crate::{
    editor::{
        ModuleUi,
        module_label::ModuleLabel,
        slider::{self, Slider},
        stereo_input::StereoInput,
        utils::confirm_module_removal,
    },
    synth_engine::{
        Input, ModuleId,
        filters::spectral_filter::FilterType,
        spectral_filter::SpectralFilterUiBridge,
        ui_bridge::{ModuleBridge, UiBridge},
    },
    utils::from_st,
};

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

    fn paint_ui(
        &mut self,
        bridge: &mut UiBridge,
        filter_bridge: &mut SpectralFilterUiBridge,
        ui: &mut Ui,
    ) {
        let module_id = self.module_id;
        let mut config = filter_bridge.config().clone();

        ui.add(ModuleLabel::new(&mut self.label_state, bridge, module_id));

        ui.add_space(20.0);

        let right_label = |ui: &mut Ui, text: &str| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(text);
            });
        };

        Grid::new("sf_grid")
            .num_columns(4)
            .spacing([8.0, 8.0])
            .show(ui, |ui| {
                right_label(ui, "Type");
                ComboBox::from_id_salt("spectral-filter-type")
                    .selected_text(config.filter_type.label())
                    .show_ui(ui, |ui| {
                        for filter_type in FilterType::ALL {
                            if ui
                                .selectable_value(
                                    &mut config.filter_type,
                                    filter_type,
                                    filter_type.label(),
                                )
                                .clicked()
                            {
                                filter_bridge.set_filter_type(filter_type);
                            }
                        }
                    });

                right_label(ui, "Cutoff");
                if ui
                    .add(StereoInput::new(
                        Input::Cutoff,
                        module_id,
                        &mut config.cutoff,
                        bridge,
                    ))
                    .changed()
                {
                    filter_bridge.set_param(Input::Cutoff, config.cutoff);
                }
                ui.end_row();

                right_label(ui, "Resonance");
                if ui
                    .add(StereoInput::new(
                        Input::Resonance,
                        module_id,
                        &mut config.resonance,
                        bridge,
                    ))
                    .changed()
                {
                    filter_bridge.set_param(Input::Resonance, config.resonance);
                }

                right_label(ui, "Drive");
                if ui
                    .add(StereoInput::new(
                        Input::Drive,
                        module_id,
                        &mut config.drive,
                        bridge,
                    ))
                    .changed()
                {
                    filter_bridge.set_param(Input::Drive, config.drive);
                }
                ui.end_row();

                right_label(ui, "Q Limit");
                if ui
                    .add(
                        Slider::stereo(&mut config.q_limit_to, 0.0..=10.0, None)
                            .default(from_st(12.0))
                            .units(slider::Units::Octaves),
                    )
                    .changed()
                {
                    filter_bridge.set_q_limit_to(config.q_limit_to);
                }

                right_label(ui, "Q Curve");
                if ui
                    .add(Slider::stereo(&mut config.q_limit_curve, 0.0..=1.0, None).default(0.5))
                    .changed()
                {
                    filter_bridge.set_q_limit_curve(config.q_limit_curve);
                }
                ui.end_row();

                right_label(ui, "Linear");
                if ui
                    .add(Checkbox::without_text(&mut config.linear_phase))
                    .changed()
                {
                    filter_bridge.set_linear_phase(config.linear_phase);
                }
                ui.end_row();
            });

        ui.add_space(40.0);

        if confirm_module_removal(ui, &mut self.remove_confirmation) {
            bridge.remove_module(module_id);
        }
    }
}

impl ModuleUi for SpectralFilterUI {
    fn module_id(&self) -> Option<ModuleId> {
        Some(self.module_id)
    }

    fn ui(&mut self, bridge: &mut UiBridge, ui: &mut Ui) {
        bridge.with_module_bridge(self.module_id, |bridge, module_bridge| {
            if let ModuleBridge::SpectralFilter(filter_bridge) = module_bridge {
                self.paint_ui(bridge, filter_bridge, ui);
            }
        });
    }
}
