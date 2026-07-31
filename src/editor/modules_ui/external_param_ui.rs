use egui::{Checkbox, ComboBox, Grid, Ui};

use crate::{
    editor::{
        ModuleUi,
        module_label::ModuleLabel,
        slider::{self, Slider},
    },
    synth_engine::{
        ModuleId, ModuleType,
        external_param::{ExternalParamUiBridge, NUM_FLOAT_PARAMS},
        ui_bridge::{ModuleBridge, UiBridge},
    },
};

pub struct ExternalParamUI {
    module_id: ModuleId,
}

impl ExternalParamUI {
    pub fn new(module_id: ModuleId) -> Self {
        Self { module_id }
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
            module_id,
            ModuleType::ExternalParam,
            bridge,
        ));

        ui.add_space(16.0);

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

                ui.label("Smooth");
                if ui
                    .add(
                        Slider::mono(&mut config.smooth, 0.0..=0.05, None)
                            .default(4.0)
                            .skew(1.2)
                            .units(slider::Units::Time),
                    )
                    .changed()
                {
                    param_bridge.set_smooth(config.smooth);
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
