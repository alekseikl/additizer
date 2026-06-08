use rustc_hash::FxHashMap;

use crate::synth_engine::{
    ModuleId, ModuleType, RoutingMap, StereoSample, SynthModule, routing::DataType,
    synth_module::ModInput,
};

pub struct Module {
    pub id: ModuleId,
    pub module_type: ModuleType,
    pub inputs: &'static [ModInput],
    pub output: DataType,
}

impl Module {
    pub fn new(module: &dyn SynthModule) -> Self {
        Self {
            id: module.id(),
            module_type: module.module_type(),
            inputs: module.inputs(),
            output: module.output(),
        }
    }
}

pub struct AvailableInputSource {
    pub src: ModuleId,
    pub label: String,
}

pub struct InputModulation {
    #[allow(unused)]
    pub src: ModuleId,
    pub label: String,
}

pub struct ConnectedInputSource {
    pub src: ModuleId,
    pub amount: StereoSample,
    pub label: String,
    pub modulation: Option<InputModulation>,
}

pub struct RoutingState {
    pub modules: FxHashMap<ModuleId, Module>,
    pub routing: RoutingMap,
}

impl RoutingState {
    pub fn new(modules: FxHashMap<ModuleId, Module>, routing: RoutingMap) -> Self {
        Self { modules, routing }
    }
}
