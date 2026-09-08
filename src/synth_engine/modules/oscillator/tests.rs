use super::{Oscillator, OscillatorConfig, OscillatorUiBridge, SynthModule};
use crate::synth_engine::{
    EngineConfig, EngineParams, Input, LinkConfig, ModuleConfig, ModuleId, Note, OUTPUT_MODULE_ID,
    Sample, SynthEngine, harmonic_editor::HarmonicEditorConfig, routing::RIGHT_CHANNEL,
};

const SAMPLE_RATE: Sample = 48_000.0;
const HARMONIC_EDITOR_ID: ModuleId = 1;
const OSCILLATOR_ID: ModuleId = 2;

fn make_engine(he: HarmonicEditorConfig, mono_spectrum: bool) -> SynthEngine {
    let config = EngineConfig {
        engine: EngineParams::default(),
        modules: vec![
            ModuleConfig::HarmonicEditor(Box::new(he)),
            ModuleConfig::Oscillator(Box::new(OscillatorConfig {
                id: OSCILLATOR_ID,
                mono_spectrum,
                ..OscillatorConfig::default()
            })),
        ],
        links: vec![
            LinkConfig::direct(HARMONIC_EDITOR_ID, OSCILLATOR_ID, Input::Spectrum),
            LinkConfig::direct(OSCILLATOR_ID, OUTPUT_MODULE_ID, Input::Audio),
        ],
    };

    SynthEngine::try_new(&config, SAMPLE_RATE).expect("valid engine config")
}

fn process_block(engine: &mut SynthEngine, samples: usize) -> (Vec<Sample>, Vec<Sample>) {
    let mut left = vec![0.0; samples];
    let mut right = vec![0.0; samples];
    let mut terminated = Vec::new();

    engine.process(
        samples,
        false,
        &mut terminated,
        [&mut left[..], &mut right[..]],
    );

    (left, right)
}

fn play(engine: &mut SynthEngine) -> (Vec<Sample>, Vec<Sample>) {
    engine.handle_note_on(
        Note {
            channel: 0,
            note: 60,
            velocity: 1.0,
            host_id: None,
        },
        0,
    );
    process_block(engine, 64);
    process_block(engine, 64)
}

fn rms(samples: &[Sample]) -> Sample {
    (samples.iter().map(|s| s * s).sum::<Sample>() / samples.len() as Sample).sqrt()
}

fn max_abs_diff(a: &[Sample], b: &[Sample]) -> Sample {
    a.iter()
        .zip(b)
        .map(|(x, y)| (x - y).abs())
        .fold(0.0, Sample::max)
}

fn silent_right_harmonics() -> HarmonicEditorConfig {
    let mut he = HarmonicEditorConfig {
        id: HARMONIC_EDITOR_ID,
        ..HarmonicEditorConfig::default()
    };

    for amp in &mut he.amplitudes[RIGHT_CHANNEL] {
        *amp = 0.0;
    }

    he
}

fn inverted_right_harmonics() -> HarmonicEditorConfig {
    let mut he = HarmonicEditorConfig {
        id: HARMONIC_EDITOR_ID,
        ..HarmonicEditorConfig::default()
    };

    for phase in &mut he.phases[RIGHT_CHANNEL] {
        *phase = (*phase + 0.5) % 1.0;
    }

    he
}

#[test]
fn mono_spectrum_defaults_to_false() {
    assert!(!OscillatorConfig::default().mono_spectrum);
    assert!(!Oscillator::new(1).get_config().mono_spectrum);
}

#[test]
fn mono_spectrum_defaults_when_missing_from_json() {
    let mut json = serde_json::to_value(OscillatorConfig::default()).unwrap();
    json.as_object_mut().unwrap().remove("mono_spectrum");

    let config: OscillatorConfig = serde_json::from_value(json).unwrap();

    assert!(!config.mono_spectrum);
}

#[test]
fn mono_spectrum_round_trips_through_config() {
    let osc = Oscillator::from_config(&OscillatorConfig {
        id: 1,
        mono_spectrum: true,
        ..OscillatorConfig::default()
    });

    assert!(osc.get_config().mono_spectrum);
}

#[test]
fn set_mono_spectrum_updates_config() {
    let mut osc = Oscillator::new(1);

    osc.set_mono_spectrum(true);

    assert!(osc.get_config().mono_spectrum);
}

#[test]
fn ui_event_sets_mono_spectrum() {
    let mut osc = Oscillator::new(1);
    let mut bridge = OscillatorUiBridge::try_new(&mut osc).expect("ui end");

    bridge.set_mono_spectrum(true);
    osc.process_ui_events();

    assert!(bridge.config().mono_spectrum);
    assert!(osc.get_config().mono_spectrum);
}

#[test]
fn stereo_spectrum_keeps_silent_right_channel_silent() {
    let mut engine = make_engine(silent_right_harmonics(), false);
    let (left, right) = play(&mut engine);

    assert!(rms(&left) > 1e-3);
    assert!(rms(&right) < 1e-6);
}

#[test]
fn mono_spectrum_reuses_left_waveform_when_right_spectrum_is_silent() {
    let mut engine = make_engine(silent_right_harmonics(), true);
    let (left, right) = play(&mut engine);

    assert!(rms(&left) > 1e-3);
    assert!(rms(&right) > 1e-3);
    assert!(max_abs_diff(&left, &right) < 1e-5);
}

#[test]
fn stereo_spectrum_keeps_inverted_right_channel_independent() {
    let mut engine = make_engine(inverted_right_harmonics(), false);
    let (left, right) = play(&mut engine);

    assert!(rms(&left) > 1e-3);
    assert!(left.iter().zip(&right).all(|(l, r)| (l + r).abs() < 1e-4));
}

#[test]
fn mono_spectrum_ignores_inverted_right_spectrum() {
    let mut engine = make_engine(inverted_right_harmonics(), true);
    let (left, right) = play(&mut engine);

    assert!(rms(&left) > 1e-3);
    assert!(max_abs_diff(&left, &right) < 1e-5);
}
