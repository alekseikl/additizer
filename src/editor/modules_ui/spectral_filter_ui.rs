use egui::{Checkbox, ComboBox, Grid, Ui};

use crate::{
    editor::{
        ModuleUi, module_label::ModuleLabel, slider::Slider, stereo_input::StereoInput,
        units::Units,
    },
    synth_engine::{
        Input, ModuleId, ModuleType,
        filters::spectral_filter::FilterType,
        spectral_filter::SpectralFilterUiBridge,
        ui_bridge::{ModuleBridge, UiBridge},
    },
    utils::from_st,
};

pub struct SpectralFilterUI {
    module_id: ModuleId,
}

impl SpectralFilterUI {
    pub fn new(module_id: ModuleId) -> Self {
        Self { module_id }
    }

    fn paint_ui(
        &mut self,
        bridge: &mut UiBridge,
        filter_bridge: &mut SpectralFilterUiBridge,
        ui: &mut Ui,
    ) {
        let module_id = self.module_id;
        let mut config = filter_bridge.config().clone();

        ui.add(ModuleLabel::new(
            module_id,
            ModuleType::SpectralFilter,
            bridge,
        ));

        ui.add_space(16.0);

        Grid::new("sf_grid")
            .num_columns(4)
            .spacing([8.0, 8.0])
            .show(ui, |ui| {
                ui.label("Type");
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

                ui.label("Cutoff");
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

                ui.label("Resonance");
                ui.horizontal(|ui| {
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

                    ui.add_space(8.0);
                });

                ui.label("Drive");
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

                ui.label("Q Limit");
                if ui
                    .add(
                        Slider::stereo(&mut config.q_limit_to, 0.0..=10.0, None)
                            .default(from_st(12.0))
                            .units(Units::Octaves),
                    )
                    .changed()
                {
                    filter_bridge.set_q_limit_to(config.q_limit_to);
                }

                ui.label("Q Curve");
                if ui
                    .add(Slider::stereo(&mut config.q_limit_curve, 0.0..=1.0, None).default(0.5))
                    .changed()
                {
                    filter_bridge.set_q_limit_curve(config.q_limit_curve);
                }
                ui.end_row();

                ui.label("Linear");
                if ui
                    .add(Checkbox::without_text(&mut config.linear_phase))
                    .changed()
                {
                    filter_bridge.set_linear_phase(config.linear_phase);
                }
                ui.end_row();
            });
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
