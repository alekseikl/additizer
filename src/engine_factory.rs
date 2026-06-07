use std::sync::{
    Arc,
    atomic::{AtomicI64, Ordering},
};

use nih_plug::params::FloatParam;
use parking_lot::Mutex;

use crate::synth_engine::{ExternalParamsBlock, FullConfig, Sample, SynthEngine};

pub type EngineHandle = Arc<Mutex<SynthEngine>>;

pub struct EngineFactory {
    external_params: Arc<ExternalParamsBlock>,
    output_level_param: Arc<FloatParam>,
    host_sample_rate: Sample,
    current: Mutex<EngineHandle>,
    seq_idx: AtomicI64,
}

impl EngineFactory {
    pub fn new(
        output_level_param: Arc<FloatParam>,
        external_params: Arc<ExternalParamsBlock>,
        host_sample_rate: Sample,
    ) -> Self {
        Self {
            external_params: external_params.clone(),
            output_level_param: output_level_param.clone(),
            host_sample_rate,
            current: Mutex::new(Arc::new(Mutex::new(
                SynthEngine::try_new(
                    &FullConfig::default(),
                    output_level_param.clone(),
                    external_params.clone(),
                    host_sample_rate,
                )
                .unwrap(),
            ))),
            seq_idx: AtomicI64::new(1),
        }
    }

    pub fn get_engine(&self) -> EngineHandle {
        self.current.lock().clone()
    }

    pub fn get_seq_idx(&self) -> i64 {
        self.seq_idx.load(Ordering::Acquire)
    }

    pub fn load_config(&self, cfg: &FullConfig) -> bool {
        let Some(new_engine) = SynthEngine::try_new(
            cfg,
            self.output_level_param.clone(),
            self.external_params.clone(),
            self.host_sample_rate,
        ) else {
            return false;
        };

        *self.current.lock() = Arc::new(Mutex::new(new_engine));
        self.seq_idx.fetch_add(1, Ordering::AcqRel);
        true
    }
}
