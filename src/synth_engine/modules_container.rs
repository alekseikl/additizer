use std::collections::HashMap;

use crate::synth_engine::{routing::ModuleId, synth_module::SynthModule};

type Container<T> = HashMap<ModuleId, Option<Box<T>>>;

pub struct ModulesContainer<T: SynthModule> {
    pub modules: Container<T>,
}

impl<T: SynthModule> ModulesContainer<T> {
    pub fn new() -> Self {
        Self {
            modules: HashMap::new(),
        }
    }

    pub fn add(&mut self, module: T) {
        self.modules.insert(module.get_id(), Some(Box::new(module)));
    }

    #[inline]
    pub fn get(&self, id: ModuleId) -> Option<&T> {
        self.modules.get(&id)?.as_deref()
    }

    #[inline]
    pub fn get_mut(&mut self, id: ModuleId) -> Option<&mut T> {
        self.modules.get_mut(&id)?.as_deref_mut()
    }

    #[inline]
    pub fn take(&mut self, id: ModuleId) -> Box<T> {
        self.modules.get_mut(&id).unwrap().take().unwrap()
    }

    #[inline]
    pub fn return_back(&mut self, module: Box<T>) {
        self.modules
            .get_mut(&module.get_id())
            .unwrap()
            .replace(module);
    }
}

impl<T: SynthModule> Default for ModulesContainer<T> {
    fn default() -> Self {
        Self::new()
    }
}
