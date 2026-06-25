use rustc_hash::{FxHashMap, FxHashSet};

use crate::synth_engine::{
    InputId, ModuleHandle, ModuleId, ModuleType, RoutingMap, StereoSample,
    routing::{DataType, InputMeta, InputSource},
    synth_module::SynthModule,
};

pub struct Module {
    pub id: ModuleId,
    pub module_type: ModuleType,
    pub inputs: &'static [InputMeta],
    pub output_type: DataType,
}

impl Module {
    pub(in super::super) fn new(module: &ModuleHandle) -> Self {
        Self {
            id: module.id(),
            module_type: module.module_type(),
            inputs: module.inputs(),
            output_type: module.output_type(),
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

pub struct ModuleInput {
    pub meta: InputMeta,
    pub sources: Vec<InputSource>,
}

// Module inputs/outputs supposed to be owned by widget
pub struct ModuleIo {
    pub id: ModuleId,
    pub module_type: ModuleType,
    pub inputs_meta: &'static [InputMeta],
    pub inputs: Vec<ModuleInput>,
    pub output_type: DataType,
    pub output_connected: bool,
}

pub struct RoutingState {
    pub modules: FxHashMap<ModuleId, Module>,
    pub routing: RoutingMap,
    pub modules_io: Option<FxHashMap<ModuleId, ModuleIo>>,
}

impl RoutingState {
    pub fn new(modules: FxHashMap<ModuleId, Module>, routing: RoutingMap) -> Self {
        let connected_outputs: FxHashSet<ModuleId> = routing
            .values()
            .flat_map(|sources| sources.iter().flat_map(InputSource::source_ids))
            .collect();

        let modules_io: FxHashMap<ModuleId, ModuleIo> = modules
            .values()
            .map(|m| {
                (
                    m.id,
                    ModuleIo {
                        id: m.id,
                        module_type: m.module_type,
                        output_type: m.output_type,
                        inputs_meta: m.inputs,
                        inputs: m
                            .inputs
                            .iter()
                            .filter_map(|meta| {
                                routing
                                    .get(&InputId::new(meta.input_type, m.id))
                                    .map(|s| (meta, s))
                            })
                            .map(|(&meta, s)| ModuleInput {
                                meta,
                                sources: s.clone(),
                            })
                            .collect(),
                        output_connected: connected_outputs.contains(&m.id),
                    },
                )
            })
            .collect();

        Self {
            modules,
            routing,
            modules_io: Some(modules_io),
        }
    }
}
