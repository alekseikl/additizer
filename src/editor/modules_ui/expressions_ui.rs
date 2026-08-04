use egui::{Checkbox, ComboBox, Grid, Ui};

use crate::{
    editor::{
        ModuleUi,
        module_label::ModuleLabel,
        slider::{self, Slider},
    },
    synth_engine::{
        Expression, ModuleId, ModuleType,
        expressions::ExpressionsUiBridge,
        ui_bridge::{ModuleBridge, UiBridge},
    },
    utils::from_ms,
};

impl Expression {
    pub fn label(&self) -> &'static str {
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

        Grid::new("expressions-grid")
            .num_columns(4)
            .spacing([8.0, 8.0])
            .show(ui, |ui| {
                ui.label("Expression");
                ui.horizontal(|ui| {
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

                    ui.add_space(8.0);
                });

                ui.label("Smooth");
                if ui
                    .add(
                        Slider::mono(&mut config.smooth, 0.0..=0.05, None)
                            .default(from_ms(4.0))
                            .skew(1.2)
                            .units(slider::Units::Time),
                    )
                    .changed()
                {
                    expr_bridge.set_smooth(config.smooth);
                }
                ui.end_row();

                if matches!(config.expression, Expression::Velocity) {
                    ui.label("Use Release velocity");
                    if ui
                        .add(Checkbox::without_text(&mut config.use_release_velocity))
                        .changed()
                    {
                        expr_bridge.set_use_release_velocity(config.use_release_velocity);
                    }
                    ui.end_row();
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
