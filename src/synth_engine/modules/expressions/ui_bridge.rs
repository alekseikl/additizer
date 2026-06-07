use std::sync::Arc;

use parking_lot::Mutex;

use crate::synth_engine::{Expression, ModuleId, Sample, SynthEngine};

use super::link::UiEnd;
use super::{Expressions, ExpressionsConfig};

pub struct UiBridge {
    synth: Arc<Mutex<SynthEngine>>,
    module_id: ModuleId,
    ui_end: Option<UiEnd>,
    config: ExpressionsConfig,
}

impl UiBridge {
    pub fn create(module_id: ModuleId, synth: Arc<Mutex<SynthEngine>>) -> Option<Self> {
        let mut synth_lock = synth.lock();
        let exp = synth_lock.get_typed_module_mut::<Expressions>(module_id)?;
        let ui_end = exp.take_ui_end()?;
        let config = exp.get_config();

        drop(synth_lock);

        Some(Self {
            synth,
            module_id,
            ui_end: Some(ui_end),
            config,
        })
    }

    pub fn module_id(&self) -> ModuleId {
        self.module_id
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

impl Drop for UiBridge {
    fn drop(&mut self) {
        let mut synth_lock = self.synth.lock();

        if let Some(exp) = synth_lock.get_typed_module_mut::<Expressions>(self.module_id) {
            exp.return_ui_end(self.ui_end.take().unwrap());
        }
    }
}
