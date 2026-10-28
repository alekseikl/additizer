use addi_ui_backend::ui_bridge::modules::expressions::ExpressionsUiBridge;
use egui::{Checkbox, ComboBox, Ui};

use crate::{ModuleUi, module_label::ModuleLabel};
use addi_engine::{Expression, ModuleId, ModuleType};
use addi_ui_backend::ui_bridge::{ModuleBridge, UiBridge};

trait ExpressionLabel {
    fn label(&self) -> &'static str;
}

impl ExpressionLabel for Expression {
    fn label(&self) -> &'static str {
        match self {
            Self::Velocity => "Velocity",
            Self::Gain => "Gain",
            Self::Pan => "Pan",
            Self::Pitch => "Pitch",
            Self::Timbre => "Timbre",
            Self::Pressure => "Pressure",
        }
    }
}

pub struct ExpressionsUi {
    module_id: ModuleId,
}

impl ExpressionsUi {
    pub fn new(module_id: ModuleId) -> Self {
        Self { module_id }
    }

    fn paint_ui(
        &mut self,
        bridge: &mut UiBridge,
        expr_bridge: &mut ExpressionsUiBridge,
        ui: &mut Ui,
    ) {
        let module_id = self.module_id;
        let mut config = expr_bridge.config().clone();

        ui.add(ModuleLabel::new(module_id, ModuleType::Expressions, bridge));

        ui.add_space(16.0);

        ui.horizontal(|ui| {
            ui.label("Expression");
            ComboBox::from_id_salt("expressions-combo")
                .selected_text(config.expression.label())
                .show_ui(ui, |ui| {
                    const TYPE_OPTIONS: &[Expression] = &[
                        Expression::Velocity,
                        Expression::Gain,
                        Expression::Pan,
                        Expression::Pitch,
                        Expression::Timbre,
                        Expression::Pressure,
                    ];

                    for expression in TYPE_OPTIONS {
                        if ui
                            .selectable_value(
                                &mut config.expression,
                                *expression,
                                expression.label(),
                            )
                            .clicked()
                        {
                            expr_bridge.set_expression(*expression);
                        }
                    }
                });

            if matches!(config.expression, Expression::Velocity) {
                ui.add_space(8.0);
                if ui
                    .add(Checkbox::new(
                        &mut config.use_release_velocity,
                        "Use Release velocity",
                    ))
                    .changed()
                {
                    expr_bridge.set_use_release_velocity(config.use_release_velocity);
                }
            }
        });
    }
}

impl ModuleUi for ExpressionsUi {
    fn module_id(&self) -> Option<ModuleId> {
        Some(self.module_id)
    }

    fn ui(&mut self, bridge: &mut UiBridge, ui: &mut Ui) {
        bridge.with_module_bridge(self.module_id, |bridge, module_bridge| {
            if let ModuleBridge::Expressions(expr_bridge) = module_bridge {
                self.paint_ui(bridge, expr_bridge, ui);
            }
        });
    }
}
