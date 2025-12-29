use crate::synth_engine::{
    ModuleId, ModuleType, Sample, SynthModule,
    buffer::{Buffer, ONES_BUFFER},
    routing::{DataType, Router},
    synth_module::{InputInfo, ProcessParams},
};

pub struct One {
    id: ModuleId,
}

impl One {
    pub fn new(id: ModuleId) -> Self {
        Self { id }
    }
}

impl SynthModule for One {
    fn id(&self) -> ModuleId {
        self.id
    }

    fn label(&self) -> String {
        "One".to_string()
    }

    fn set_label(&mut self, _label: String) {}

    fn module_type(&self) -> ModuleType {
        ModuleType::One
    }

    fn inputs(&self) -> &'static [InputInfo] {
        &[]
    }

    fn output(&self) -> DataType {
        DataType::Scalar
    }

    fn process(&mut self, _params: &ProcessParams, _router: &dyn Router) {}

    fn get_buffer_output(&self, _voice_idx: usize, _channel_idx: usize) -> &Buffer {
        &ONES_BUFFER
    }

    fn get_scalar_output(&self, _current: bool, _voice_idx: usize, _channel: usize) -> Sample {
        1.0
    }
}
