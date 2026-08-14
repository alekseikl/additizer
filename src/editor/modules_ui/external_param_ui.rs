use egui::{Checkbox, ComboBox, Grid, Ui};

use crate::{
    editor::{ModuleUi, module_label::ModuleLabel, slider::Slider, units::Units},
    synth_engine::{
        ModuleId, ModuleType,
        external_param::{ExternalParamUiBridge, NUM_FLOAT_PARAMS},
        ui_bridge::{ModuleBridge, UiBridge},
    },
    utils::from_ms,
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
            .num_columns(4)
            .spacing([8.0, 8.0])
            .show(ui, |ui| {
                ui.label("Input");
                ui.horizontal(|ui| {
                    ComboBox::from_id_salt("ext-param-select")
                        .selected_text(format!("Param #{}", config.selected_param_index + 1))
                        .show_ui(ui, |ui| {
                            for i in 0..NUM_FLOAT_PARAMS {
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
                });

                ui.label("Smooth");
                if ui
                    .add(
                        Slider::mono(&mut config.smooth, 0.0..=0.05, None)
                            .default(from_ms(4.0))
                            .skew(1.2)
                            .units(Units::Time),
                    )
                    .changed()
                {
                    param_bridge.set_smooth(config.smooth);
                }
                ui.end_row();

                ui.label("Hold")
                    .on_hover_text("Hold a value copied on trigger");
                if ui
                    .add(Checkbox::without_text(&mut config.sample_on_trigger))
                    .changed()
                {
                    param_bridge.set_sample_on_trigger(config.sample_on_trigger);
                }

                ui.label("Make Bipolar")
                    .on_hover_text("Remaps [0, 1] into [-1, 1]");
                if ui
                    .add(Checkbox::without_text(&mut config.make_bipolar))
                    .changed()
                {
                    param_bridge.set_make_bipolar(config.make_bipolar);
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
