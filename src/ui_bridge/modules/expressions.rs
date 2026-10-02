use crate::synth_engine::expressions::{Expressions, ExpressionsConfig, ExpressionsLinks, ExpressionsUiEnd};
use crate::synth_engine::{Expression, Sample, synth_module::ModuleUiBridge};


pub struct ExpressionsUiBridge<L: ExpressionsLinks = crate::links::expressions::Links> {
    ui_end: L::UiEnd,
    config: ExpressionsConfig,
}

impl<L: ExpressionsLinks> ExpressionsUiBridge<L> {
    pub fn try_new(exp: &mut Expressions<L>) -> Option<Self> {
        Some(Self {
            ui_end: exp.take_ui_end()?,
            config: exp.get_config(),
        })
    }

    pub fn config(&self) -> &ExpressionsConfig {
        &self.config
    }

    pub fn get_value(&mut self) -> Sample {
        self.ui_end.get_value()
    }

    pub fn set_expression(&mut self, expression: Expression) {
        if self.ui_end.set_expression(expression) {
            self.config.expression = expression;
        }
    }

    pub fn set_use_release_velocity(&mut self, value: bool) {
        if self.ui_end.set_use_release_velocity(value) {
            self.config.use_release_velocity = value;
        }
    }

    pub fn set_smooth(&mut self, value: Sample) {
        if self.ui_end.set_smooth(value) {
            self.config.smooth = value;
        }
    }
}

impl<L: ExpressionsLinks> ModuleUiBridge for ExpressionsUiBridge<L> {
    fn update(&mut self) -> bool {
        false
    }
}
