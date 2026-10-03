#![allow(clippy::new_without_default)]

//! UI-thread backend: concrete engine links, the UI bridge, presets, and the default patch.

pub mod default_scheme;
pub mod engine_factory;
pub mod links;
pub mod preset;
pub mod presets;
pub mod ui_bridge;

mod log;
