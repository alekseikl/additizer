use std::any::Any;
use std::sync::Arc;

use nih_plug::params::FloatParam;
use serde::{Deserialize, Serialize};

use crate::synth_engine::{
    InputType, ModuleId, ModuleType, Sample, SynthModule,
    routing::{OutputType, Router},
    synth_module::{ModuleConfigBox, ProcessParams},
};

pub const NUM_FLOAT_PARAMS: usize = 4;

pub struct ExternalParamsBlock {
    pub float_params: [Arc<FloatParam>; NUM_FLOAT_PARAMS],
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct ExternalParamConfig {
    label: Option<String>,
    selected_param_index: usize,
}

pub struct ExternalParamUI {
    pub label: String,
    pub selected_param_index: usize,
    pub num_of_params: usize,
}

pub struct ExternalParam {
    id: ModuleId,
    label: String,
    config: ModuleConfigBox<ExternalParamConfig>,
    params_block: Arc<ExternalParamsBlock>,
    selected_param: Option<Arc<FloatParam>>,
    selected_param_index: usize,
    prev_output: Sample,
    output: Sample,
}

impl ExternalParam {
    pub fn new(
        id: ModuleId,
        config: ModuleConfigBox<ExternalParamConfig>,
        params_block: Arc<ExternalParamsBlock>,
    ) -> Self {
        let mut ext = Self {
            id,
            label: format!("External Param {id}"),
            config,
            params_block,
            selected_param: None,
            selected_param_index: 0,
            prev_output: 0.0,
            output: 0.0,
        };

        {
            let cfg = ext.config.lock();

            if let Some(label) = cfg.label.as_ref() {
                ext.label = label.clone();
            }

            let idx = cfg
                .selected_param_index
                .min(ext.params_block.float_params.len() - 1);

            ext.selected_param_index = idx;
            ext.selected_param = Some(Arc::clone(&ext.params_block.float_params[idx]));
        }

        ext
    }

    gen_downcast_methods!(ExternalParam);

    pub fn get_ui(&self) -> ExternalParamUI {
        ExternalParamUI {
            label: self.label.clone(),
            selected_param_index: self.selected_param_index,
            num_of_params: NUM_FLOAT_PARAMS,
        }
    }

    pub fn select_param(&mut self, param_idx: usize) {
        let param_idx = param_idx.min(self.params_block.float_params.len() - 1);

        self.selected_param_index = param_idx;
        self.selected_param = Some(Arc::clone(&self.params_block.float_params[param_idx]));
        self.config.lock().selected_param_index = param_idx;
    }
}

impl SynthModule for ExternalParam {
    fn id(&self) -> ModuleId {
        self.id
    }

    fn label(&self) -> String {
        self.label.clone()
    }

    fn set_label(&mut self, label: String) {
        self.label = label.clone();
        self.config.lock().label = Some(label);
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::ExternalParam
    }

    fn is_spectral_rate(&self) -> bool {
        true
    }

    fn inputs(&self) -> &'static [InputType] {
        &[]
    }

    fn output_type(&self) -> OutputType {
        OutputType::Scalar
    }

    fn process(&mut self, _params: &ProcessParams, _router: &dyn Router) {
        self.prev_output = self.output;
        self.output = self
            .selected_param
            .as_ref()
            .map(|param| param.value())
            .unwrap_or_default();
    }

    fn get_scalar_output(&self, _voice_idx: usize, _channel: usize) -> (Sample, Sample) {
        (self.prev_output, self.output)
    }
}
