use crate::synth_engine::{Expression, Sample, synth_module::ModuleUiBridge};

use super::link::UiEnd;
use super::{Expressions, ExpressionsConfig};

pub struct ExpressionsUiBridge {
    ui_end: UiEnd,
    config: ExpressionsConfig,
}

impl ExpressionsUiBridge {
    pub fn try_new(exp: &mut Expressions) -> Option<Self> {
        Some(Self {
            ui_end: exp.ui_end.take()?,
            config: exp.get_config(),
        })
    }

    pub fn config(&self) -> &ExpressionsConfig {
        &self.config
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

impl ModuleUiBridge for ExpressionsUiBridge {
    fn update(&mut self) {}
}
