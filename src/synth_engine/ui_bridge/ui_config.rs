use std::ops::Add;

use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};

use crate::synth_engine::ModuleId;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct GridVec {
    pub x: i32,
    pub y: i32,
}

impl GridVec {
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

impl Add for GridVec {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct UiModuleConfig {
    pub id: ModuleId,
    pub label: String,
    #[serde(default)]
    pub position: GridVec,
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    pub modules: FxHashMap<ModuleId, UiModuleConfig>,
}
