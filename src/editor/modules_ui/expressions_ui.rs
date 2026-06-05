use egui::{Checkbox, ComboBox, Grid, Ui};

use crate::{
    editor::{
        ModuleUi, module_label::ModuleLabel, stereo_slider::StereoSlider,
        utils::confirm_module_removal,
    },
    synth_engine::{
        Expression, ModuleId, StereoSample, expressions, ui_bridge::UiBridge,
    },
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
    remove_confirmation: bool,
    label_state: Option<String>,
    expr_bridge: expressions::UiBridge,
}

impl ExpressionsUi {
    pub fn new(module_id: ModuleId, synth_bridge: &mut UiBridge) -> Option<Self> {
        let expr_bridge = expressions::UiBridge::create(module_id, synth_bridge.synth().clone())?;

        Some(Self {
            remove_confirmation: false,
            label_state: None,
            expr_bridge,
        })
    }
}

impl ModuleUi for ExpressionsUi {
    fn module_id(&self) -> Option<ModuleId> {
        Some(self.expr_bridge.module_id())
    }

    fn ui(&mut self, bridge: &mut UiBridge, ui: &mut Ui) {
        let module_id = self.expr_bridge.module_id();
        let mut config = self.expr_bridge.config().clone();

        ui.add(ModuleLabel::new(
            &mut self.label_state,
            bridge,
            module_id,
        ));

        ui.add_space(20.0);

        Grid::new("expressions-grid")
            .num_columns(2)
            .spacing([40.0, 24.0])
            .striped(true)
            .show(ui, |ui| {
                let mut smooth = StereoSample::splat(config.smooth);

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
                                self.expr_bridge.set_expression(*expression);
                            }
                        }
                    });
                ui.end_row();

                if matches!(config.expression, Expression::Velocity) {
                    ui.label("Use Release velocity");
                    if ui
                        .add(Checkbox::without_text(&mut config.use_release_velocity))
                        .changed()
                    {
                        self.expr_bridge
                            .set_use_release_velocity(config.use_release_velocity);
                    }
                    ui.end_row();
                }

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
                    self.expr_bridge.set_smooth(smooth.left());
                }
                ui.end_row();
            });

        ui.add_space(40.0);

        if confirm_module_removal(ui, &mut self.remove_confirmation) {
            bridge.remove_module(module_id);
        }
    }
}
