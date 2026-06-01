use rustc_hash::FxHashMap;

use crate::synth_engine::{
    Input, ModuleId, ModuleInput, ModuleType, OUTPUT_MODULE_ID, RoutingMap, StereoSample,
    SynthModule,
    routing::{DataType, data_types_compatible},
    synth_module::ModInput,
};

pub struct Module {
    id: ModuleId,
    module_type: ModuleType,
    label: String,
    inputs: &'static [ModInput],
    output: DataType,
}

impl Module {
    pub fn new(module: &dyn SynthModule) -> Self {
        Self {
            id: module.id(),
            module_type: module.module_type(),
            label: module.label(),
            inputs: module.inputs(),
            output: module.output(),
        }
    }
}

pub struct ModuleItem {
    pub id: ModuleId,
    pub module_type: ModuleType,
    pub label: String,
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
    modules: FxHashMap<ModuleId, Module>,
    routing: RoutingMap,
}

impl RoutingState {
    pub fn new(modules: FxHashMap<ModuleId, Module>, routing: RoutingMap) -> Self {
        Self { modules, routing }
    }

    pub fn get_modules(&self) -> Vec<ModuleItem> {
        self.modules
            .values()
            .map(|m| ModuleItem {
                id: m.id,
                module_type: m.module_type,
                label: m.label.clone(),
            })
            .collect()
    }

    pub fn get_available_input_sources(&self, input: ModuleInput) -> Vec<AvailableInputSource> {
        let dst_data_type =
            if input.module_id == OUTPUT_MODULE_ID && input.input_type == Input::Audio {
                DataType::Buffer
            } else if let Some(input_module) = self.modules.get(&input.module_id)
                && let Some(input_info) = input_module
                    .inputs
                    .iter()
                    .find(|input_info| input_info.input == input.input_type)
            {
                input_info.data_type
            } else {
                return Vec::new();
            };

        self.modules
            .values()
            .filter(|module| {
                module.id != input.module_id
                    && data_types_compatible(module.output, dst_data_type)
                    && !self.is_connected_to_source(module.id, input.module_id)
            })
            .map(|module| AvailableInputSource {
                src: module.id,
                label: module.label.clone(),
            })
            .collect()
    }

    pub fn get_connected_input_sources(&self, input: ModuleInput) -> Vec<ConnectedInputSource> {
        let Some(sources) = self.routing.get(&input) else {
            return Vec::new();
        };

        sources
            .iter()
            .filter_map(|source| self.modules.get(&source.src).map(|module| (module, source)))
            .map(|(module, source)| ConnectedInputSource {
                src: source.src,
                amount: source.amount,
                label: module.label.clone(),
                modulation: source
                    .modulation
                    .and_then(|modulation| {
                        self.modules
                            .get(&modulation.src)
                            .map(|module| (module, modulation))
                    })
                    .map(|(module, modulation)| InputModulation {
                        src: modulation.src,
                        label: module.label.clone(),
                    }),
            })
            .collect()
    }

    pub fn update_link_amount(&mut self, src: ModuleId, dst: ModuleInput, amount: StereoSample) {
        if let Some(sources) = self.routing.get_mut(&dst)
            && let Some(source) = sources.iter_mut().find(|s| s.src == src)
        {
            source.amount = amount;
        }
    }

    fn is_connected_to_source(&self, dst_id: ModuleId, src_id: ModuleId) -> bool {
        for (input, sources) in &self.routing {
            if input.module_id == dst_id {
                for source in sources.iter().flat_map(|src| src.source_ids()) {
                    if source == src_id || self.is_connected_to_source(source, src_id) {
                        return true;
                    }
                }
            }
        }

        false
    }
}
