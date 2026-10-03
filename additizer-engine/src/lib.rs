#![allow(clippy::new_without_default)]

//! Synthesizer engine: modules, routing, and voice allocation.
//!
//! UI/audio link implementations live with the plugin. The engine is generic over
//! [`synth_engine::EngineLinks`] and defaults to [`synth_engine::stub::StubLinks`].

mod synth_engine;

pub use synth_engine::*;
