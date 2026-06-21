use rustc_hash::FxHashMap;

use crate::synth_engine::{
    Input, ModuleHandle, ModuleId, ModuleType, RoutingMap, StereoSample,
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
    pub inputs: Vec<ModuleInput>,
    pub output_type: DataType,
}

pub struct RoutingState {
    pub modules: FxHashMap<ModuleId, Module>,
    pub routing: RoutingMap,
    pub modules_io: Option<FxHashMap<ModuleId, ModuleIo>>,
}

impl RoutingState {
    pub fn new(modules: FxHashMap<ModuleId, Module>, routing: RoutingMap) -> Self {
        let meta_lookup = |module_id: ModuleId, input_type: Input| -> InputMeta {
            let module = modules.get(&module_id).expect("module in place");

            *module
                .inputs
                .iter()
                .find(|meta| meta.input_type == input_type)
                .expect("input in place")
        };

        let modules_io: FxHashMap<ModuleId, ModuleIo> = modules
            .values()
            .map(|m| {
                (
                    m.id,
                    ModuleIo {
                        id: m.id,
                        module_type: m.module_type,
                        output_type: m.output_type,
                        inputs: routing
                            .iter()
                            .filter(|(id, _)| id.module_id == m.id)
                            .map(|(id, sources)| ModuleInput {
                                meta: meta_lookup(m.id, id.input_type),
                                sources: sources.clone(),
                            })
                            .collect(),
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
