use crate::synth_engine::{EngineConfig, SynthEngine};

#[test]
fn stub_engine_has_no_ui_end() {
    let mut engine: SynthEngine =
        SynthEngine::try_new(&EngineConfig::default(), 48_000.0).expect("stub engine");

    assert!(engine.take_ui_end().is_none());
}
