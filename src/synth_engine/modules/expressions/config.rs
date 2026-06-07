use serde::{Deserialize, Serialize};

use crate::{
    synth_engine::{Expression, ModuleId, Sample},
    utils::from_ms,
};

#[derive(Clone, Serialize, Deserialize)]
pub struct ExpressionsConfig {
    pub id: ModuleId,
    pub expression: Expression,
    pub use_release_velocity: bool,
    pub smooth: Sample,
}

impl Default for ExpressionsConfig {
    fn default() -> Self {
        Self {
            id: -1,
            expression: Expression::Velocity,
            use_release_velocity: false,
            smooth: from_ms(4.0),
        }
    }
}
