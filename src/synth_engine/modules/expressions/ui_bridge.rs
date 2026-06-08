use std::sync::Arc;

use parking_lot::Mutex;

use crate::synth_engine::{
    Expression, ModuleId, Sample, SynthEngine, synth_module::ModuleUiBridge,
};

use super::link::UiEnd;
use super::{Expressions, ExpressionsConfig};

pub struct ExpressionsUiBridge {
    ui_end: Option<UiEnd>,
    config: ExpressionsConfig,
}

impl ExpressionsUiBridge {
    pub fn create(module_id: ModuleId, synth: Arc<Mutex<SynthEngine>>) -> Option<Self> {
        let mut synth_lock = synth.lock();
        let exp = synth_lock.get_typed_module_mut::<Expressions>(module_id)?;
        let ui_end = exp.ui_end.take()?;
        let config = exp.get_config();

        drop(synth_lock);

        Some(Self {
            ui_end: Some(ui_end),
            config,
        })
    }

    pub fn config(&self) -> &ExpressionsConfig {
        &self.config
    }

    pub fn set_expression(&mut self, expression: Expression) {
        if self.ui_end.as_mut().unwrap().set_expression(expression) {
            self.config.expression = expression;
        }
    }

    pub fn set_use_release_velocity(&mut self, value: bool) {
        if self
            .ui_end
            .as_mut()
            .unwrap()
            .set_use_release_velocity(value)
        {
            self.config.use_release_velocity = value;
        }
    }

    pub fn set_smooth(&mut self, value: Sample) {
        if self.ui_end.as_mut().unwrap().set_smooth(value) {
            self.config.smooth = value;
        }
    }
}

impl ModuleUiBridge for ExpressionsUiBridge {
    fn update(&mut self) {}
}
