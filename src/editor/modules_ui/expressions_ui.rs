use egui_baseview::egui::{Checkbox, ComboBox, Grid, Ui};

use crate::{
    editor::{
        ModuleUi, module_label::ModuleLabel, stereo_slider::StereoSlider,
        utils::confirm_module_removal,
    },
    synth_engine::{Expression, Expressions, ModuleId, StereoSample, SynthEngine},
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
    remove_confirmation: bool,
    label_state: Option<String>,
}

impl ExpressionsUi {
    pub fn new(module_id: ModuleId) -> Self {
        Self {
            module_id,
            remove_confirmation: false,
            label_state: None,
        }
    }

    fn expr<'a>(&mut self, synth: &'a mut SynthEngine) -> &'a mut Expressions {
        Expressions::downcast_mut_unwrap(synth.get_module_mut(self.module_id))
    }
}

impl ModuleUi for ExpressionsUi {
    fn module_id(&self) -> Option<ModuleId> {
        Some(self.module_id)
    }

    fn ui(&mut self, synth: &mut SynthEngine, ui: &mut Ui) {
        let mut ui_data = self.expr(synth).get_ui();

        ui.add(ModuleLabel::new(
            &ui_data.label,
            &mut self.label_state,
            synth.get_module_mut(self.module_id).unwrap(),
        ));

        ui.add_space(20.0);

        Grid::new("expressions-grid")
            .num_columns(2)
            .spacing([40.0, 24.0])
            .striped(true)
            .show(ui, |ui| {
                let mut smooth = StereoSample::splat(ui_data.smooth);

                ui.label("Expression");
                ComboBox::from_id_salt("expressions-combo")
                    .selected_text(ui_data.expression.label())
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
                                    &mut ui_data.expression,
                                    *expression,
                                    expression.label(),
                                )
                                .clicked()
                            {
                                self.expr(synth).set_expression(*expression);
                            }
                        }
                    });
                ui.end_row();

                if matches!(ui_data.expression, Expression::Velocity) {
                    ui.label("Use Release velocity");
                    if ui
                        .add(Checkbox::without_text(&mut ui_data.use_release_velocity))
                        .changed()
                    {
                        self.expr(synth)
                            .set_use_release_velocity(ui_data.use_release_velocity);
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
                    self.expr(synth).set_smooth(smooth.left());
                }
                ui.end_row();
            });

        ui.add_space(40.0);

        if confirm_module_removal(ui, &mut self.remove_confirmation) {
            synth.remove_module(self.module_id);
        }
    }
}
