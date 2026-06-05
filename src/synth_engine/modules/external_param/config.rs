use serde::{Deserialize, Serialize};

use crate::{
    synth_engine::{ModuleId, Sample},
    utils::from_ms,
};

#[derive(Clone, Serialize, Deserialize)]
pub struct Config {
    pub id: ModuleId,
    pub selected_param_index: usize,
    pub smooth: Sample,
    pub sample_and_hold: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            id: -1,
            selected_param_index: 0,
            smooth: from_ms(2.0),
            sample_and_hold: false,
        }
    }
}
