use egui::{Checkbox, ComboBox, Grid, Ui};

use crate::{
    editor::{
        ModuleUi, module_label::ModuleLabel, stereo_slider::StereoSlider,
        utils::confirm_module_removal,
    },
    synth_engine::{
        ModuleId, StereoSample,
        external_param::{ExternalParamUiBridge, NUM_FLOAT_PARAMS},
        ui_bridge::{ModuleBridge, UiBridge},
    },
};

pub struct ExternalParamUI {
    module_id: ModuleId,
    remove_confirmation: bool,
    label_state: Option<String>,
}

impl ExternalParamUI {
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
        param_bridge: &mut ExternalParamUiBridge,
        ui: &mut Ui,
    ) {
        let module_id = self.module_id;
        let mut config = param_bridge.config().clone();

        ui.add(ModuleLabel::new(
            &mut self.label_state,
            bridge,
            module_id,
        ));

        ui.add_space(20.0);

        Grid::new("ext-param-grid")
            .num_columns(2)
            .spacing([40.0, 24.0])
            .striped(true)
            .show(ui, |ui| {
                ui.label("Input");
                ComboBox::from_id_salt("ext-param-select")
                    .selected_text(format!("Param #{}", config.selected_param_index + 1))
                    .show_ui(ui, |ui| {
                        for i in 0..NUM_FLOAT_PARAMS {
                            if ui
                                .selectable_label(
                                    i == config.selected_param_index,
                                    format!("Param #{}", i + 1),
                                )
                                .clicked()
                            {
                                param_bridge.select_param(i);
                            }
                        }
                    });
                ui.end_row();

                let mut smooth = StereoSample::splat(config.smooth);

                ui.label("Smooth");
                if ui
                    .add(
                        StereoSlider::new(&mut smooth)
                            .range(0.0..=0.05)
                            .display_scale(1000.0)
                            .default_value(0.0)
                            .skew(1.2)
                            .precision(1)
                            .units(" ms"),
                    )
                    .changed()
                {
                    param_bridge.set_smooth(smooth.left());
                }
                ui.end_row();

                ui.label("Sample and Hold");
                if ui
                    .add(Checkbox::without_text(&mut config.sample_and_hold))
                    .changed()
                {
                    param_bridge.set_sample_and_hold(config.sample_and_hold);
                }
                ui.end_row();
            });

        ui.add_space(40.0);

        if confirm_module_removal(ui, &mut self.remove_confirmation) {
            bridge.remove_module(module_id);
        }
    }
}

impl ModuleUi for ExternalParamUI {
    fn module_id(&self) -> Option<ModuleId> {
        Some(self.module_id)
    }

    fn ui(&mut self, bridge: &mut UiBridge, ui: &mut Ui) {
        bridge.with_module_bridge(self.module_id, |bridge, module_bridge| {
            if let ModuleBridge::ExternalParam(param_bridge) = module_bridge {
                self.paint_ui(bridge, param_bridge, ui);
            }
        });
    }
}
