use addi_ui_backend::ui_bridge::modules::external_param::ExternalParamUiBridge;
use egui::{Checkbox, ComboBox, Ui};

use crate::{ModuleUi, module_label::ModuleLabel};
use addi_engine::{ModuleId, ModuleType, external_param::NUM_EXT_PARAMS};
use addi_ui_backend::ui_bridge::{ModuleBridge, UiBridge};

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

        ui.horizontal(|ui| {
            ui.label("Input");
            ComboBox::from_id_salt("ext-param-select")
                .selected_text(format!("Param #{}", config.selected_param_index + 1))
                .show_ui(ui, |ui| {
                    for i in 0..NUM_EXT_PARAMS {
                        if ui
                            .selectable_value(
                                &mut config.selected_param_index,
                                i,
                                format!("Param #{}", i + 1),
                            )
                            .clicked()
                        {
                            param_bridge.select_param(i);
                        }
                    }
                });

            ui.add_space(8.0);
            if ui
                .add(Checkbox::new(&mut config.sample_on_trigger, "Hold"))
                .on_hover_text("Hold a value copied on trigger")
                .changed()
            {
                param_bridge.set_sample_on_trigger(config.sample_on_trigger);
            }

            ui.add_space(8.0);
            if ui
                .add(Checkbox::new(&mut config.polyphonic, "Polyphonic"))
                .on_hover_text("Apply per-voice poly modulation from the host")
                .changed()
            {
                param_bridge.set_polyphonic(config.polyphonic);
            }
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
