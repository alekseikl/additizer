use addi_ui_backend::ui_bridge::modules::svf::SvfUiBridge;
use egui::{ComboBox, Grid, Ui};

use crate::{ModuleUi, module_label::ModuleLabel, slider::Slider, stereo_input::StereoInput};
use addi_dsp::filters::svf::SvfType;
use addi_engine::{Input, ModuleId, ModuleType};
use addi_ui_backend::ui_bridge::{ModuleBridge, UiBridge};

pub struct SvfUi {
    module_id: ModuleId,
}

impl SvfUi {
    pub fn new(module_id: ModuleId) -> Self {
        Self { module_id }
    }

    fn paint_ui(&mut self, bridge: &mut UiBridge, svf_bridge: &mut SvfUiBridge, ui: &mut Ui) {
        let module_id = self.module_id;
        let mut config = svf_bridge.config().clone();

        ui.add(ModuleLabel::new(module_id, ModuleType::Svf, bridge));

        ui.add_space(16.0);

        Grid::new("svf_grid")
            .num_columns(4)
            .spacing([8.0, 8.0])
            .show(ui, |ui| {
                ui.label("Type");
                ComboBox::from_id_salt("svf-type")
                    .selected_text(config.filter_type.label())
                    .show_ui(ui, |ui| {
                        for filter_type in SvfType::ALL {
                            if ui
                                .selectable_value(
                                    &mut config.filter_type,
                                    filter_type,
                                    filter_type.label(),
                                )
                                .clicked()
                            {
                                svf_bridge.set_filter_type(filter_type);
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
                    svf_bridge.set_param(Input::Cutoff, config.cutoff);
                }
                ui.end_row();

                ui.label("Resonance");
                if ui
                    .add(StereoInput::new(
                        Input::Resonance,
                        module_id,
                        &mut config.resonance,
                        bridge,
                    ))
                    .changed()
                {
                    svf_bridge.set_param(Input::Resonance, config.resonance);
                }

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
                    svf_bridge.set_param(Input::Drive, config.drive);
                }
                ui.end_row();

                ui.label("Keytrack");
                if ui
                    .add(Slider::mono(&mut config.keytrack, 0.0..=1.0, None).default(0.0))
                    .changed()
                {
                    svf_bridge.set_keytrack(config.keytrack);
                }
                ui.end_row();
            });
    }
}

impl ModuleUi for SvfUi {
    fn module_id(&self) -> Option<ModuleId> {
        Some(self.module_id)
    }

    fn ui(&mut self, bridge: &mut UiBridge, ui: &mut Ui) {
        bridge.with_module_bridge(self.module_id, |bridge, module_bridge| {
            if let ModuleBridge::Svf(svf_bridge) = module_bridge {
                self.paint_ui(bridge, svf_bridge, ui);
            }
        });
    }
}
