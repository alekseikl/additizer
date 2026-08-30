use std::sync::Arc;

use parking_lot::Mutex;

use crate::{
    engine_factory::{EngineHandle, UiConfigHandle},
    synth_engine::{
        harmonic_editor::HarmonicEditorConfig,
        oscillator::OscillatorConfig,
        ui_bridge::{
            ui_config::{UiConfig, UiModuleConfig},
            UiBridge,
        },
        EngineConfig, EngineParams, Input, InputId, LinkConfig, ModuleConfig, ModuleId, Sample,
        StereoSample, SynthEngine, OUTPUT_MODULE_ID,
    },
};

const SAMPLE_RATE: Sample = 48_000.0;
const HARMONIC_EDITOR_ID: ModuleId = 1;
const OSCILLATOR_ID: ModuleId = 2;

fn minimal_engine() -> SynthEngine {
    let config = EngineConfig {
        engine: EngineParams::default(),
        modules: vec![
            ModuleConfig::HarmonicEditor(Box::new(HarmonicEditorConfig {
                id: HARMONIC_EDITOR_ID,
                ..HarmonicEditorConfig::default()
            })),
            ModuleConfig::Oscillator(Box::new(OscillatorConfig {
                id: OSCILLATOR_ID,
                ..OscillatorConfig::default()
            })),
        ],
        links: vec![
            LinkConfig::direct(HARMONIC_EDITOR_ID, OSCILLATOR_ID, Input::Spectrum),
            LinkConfig::direct(OSCILLATOR_ID, OUTPUT_MODULE_ID, Input::Audio),
        ],
    };

    SynthEngine::try_new(&config, SAMPLE_RATE).expect("valid engine")
}

fn make_bridge(engine: SynthEngine) -> UiBridge {
    let mut ui_config = UiConfig::default();
    for (id, label) in [
        (HARMONIC_EDITOR_ID, "Harmonics"),
        (OSCILLATOR_ID, "Oscillator"),
        (OUTPUT_MODULE_ID, "Output"),
    ] {
        ui_config.modules.insert(
            id,
            UiModuleConfig {
                id,
                label: label.into(),
                ..UiModuleConfig::default()
            },
        );
    }

    let engine: EngineHandle = Arc::new(Mutex::new(engine));
    let ui_config: UiConfigHandle = Arc::new(Mutex::new(ui_config));

    UiBridge::create(engine, ui_config).expect("ui bridge takes ui_end")
}

#[test]
fn has_linkable_input_rejects_self() {
    let bridge = make_bridge(minimal_engine());

    assert!(!bridge.has_linkable_input(OSCILLATOR_ID, OSCILLATOR_ID));
}

#[test]
fn has_linkable_input_false_when_only_compatible_input_already_taken_by_src() {
    let bridge = make_bridge(minimal_engine());

    // HE is spectral-only into osc; Spectrum is already HE -> Osc, so no remaining input.
    assert!(!bridge.has_linkable_input(HARMONIC_EDITOR_ID, OSCILLATOR_ID));
}

#[test]
fn has_linkable_input_true_for_replacement_direct_source() {
    let mut engine = minimal_engine();
    let he2 = engine.add_harmonic_editor();
    let bridge = make_bridge(engine);

    assert!(bridge.has_linkable_input(he2, OSCILLATOR_ID));
}

#[test]
fn get_linkable_inputs_excludes_already_connected() {
    let bridge = make_bridge(minimal_engine());

    let linkable = bridge.get_linkable_inputs(HARMONIC_EDITOR_ID, OSCILLATOR_ID);
    assert!(
        linkable
            .iter()
            .all(|input| input.input_type != Input::Spectrum),
        "connected harmonic editor must not appear as available for Spectrum"
    );
}

#[test]
fn get_linkable_inputs_includes_alternate_direct_source() {
    let mut engine = minimal_engine();
    let he2 = engine.add_harmonic_editor();
    let bridge = make_bridge(engine);

    let linkable = bridge.get_linkable_inputs(he2, OSCILLATOR_ID);
    assert!(
        linkable
            .iter()
            .any(|input| input.input_type == Input::Spectrum),
        "second harmonic editor can replace Spectrum source"
    );
}

#[test]
fn get_linkable_inputs_excludes_self_and_type_mismatch() {
    let bridge = make_bridge(minimal_engine());

    assert!(bridge
        .get_linkable_inputs(OSCILLATOR_ID, OSCILLATOR_ID)
        .is_empty());
    assert!(bridge
        .get_linkable_inputs(OUTPUT_MODULE_ID, OSCILLATOR_ID)
        .iter()
        .all(|input| input.input_type != Input::Spectrum));
}

#[test]
fn create_link_routes_direct_spectrum() {
    let mut engine = minimal_engine();
    let he2 = engine.add_harmonic_editor();
    let mut bridge = make_bridge(engine);
    let spectrum = InputId::new(Input::Spectrum, OSCILLATOR_ID);

    bridge.create_link(he2, spectrum);

    let connected = bridge.get_connected_input_sources(spectrum);
    assert_eq!(connected.len(), 1);
    assert_eq!(connected[0].src, he2);
    assert!(connected[0].modulation.is_none());
}

#[test]
fn create_link_routes_mixed_gain() {
    let mut engine = minimal_engine();
    let lfo_id = engine.add_lfo();
    let mut bridge = make_bridge(engine);
    let gain = InputId::new(Input::Gain, OSCILLATOR_ID);

    bridge.create_link(lfo_id, gain);

    let connected = bridge.get_connected_input_sources(gain);
    assert_eq!(connected.len(), 1);
    assert_eq!(connected[0].src, lfo_id);
}

#[test]
fn get_available_mixed_excludes_connected_source() {
    let mut engine = minimal_engine();
    let lfo_id = engine.add_lfo();
    engine
        .add_mixed_link(
            lfo_id,
            InputId::new(Input::Gain, OSCILLATOR_ID),
            StereoSample::ONE,
        )
        .expect("lfo -> gain");

    let bridge = make_bridge(engine);

    let linkable = bridge.get_linkable_inputs(lfo_id, OSCILLATOR_ID);
    assert!(linkable.iter().all(|input| input.input_type != Input::Gain));
}

#[test]
fn has_linkable_input_false_for_spectral_into_audio_only_module() {
    let bridge = make_bridge(minimal_engine());

    assert!(!bridge.has_linkable_input(HARMONIC_EDITOR_ID, OUTPUT_MODULE_ID));
}
