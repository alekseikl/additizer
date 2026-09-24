use super::*;
use crate::synth_engine::{
    ComplexSample, DataType, EngineConfig, EngineParams, Input, LinkConfig, ModuleConfig, ModuleId,
    NUM_CHANNELS, Note, OUTPUT_MODULE_ID, Sample, SynthEngine, oscillator::OscillatorConfig,
    routing::LEFT_CHANNEL, synth_module::SynthModule,
};

const SAMPLE_RATE: Sample = 48_000.0;
const SRC_ID: ModuleId = 1;
const DST_ID: ModuleId = 2;

#[test]
fn frequency_bin_round_trips_amplitude_and_phase() {
    for (idx, amp, phase) in [
        (1, 1.0, 0.0),
        (2, 0.25, 0.5),
        (7, 0.8, 0.3),
        (16, 1.0, 0.75),
    ] {
        let (got_amp, got_phase) =
            HarmonicEditor::from_frequency_bin(idx, HarmonicEditor::frequency_bin(idx, amp, phase));

        assert!(
            (got_amp - amp).abs() < 1e-5,
            "amp idx {idx}: {got_amp} != {amp}"
        );
        assert!(
            (got_phase - phase).abs() < 1e-5,
            "phase idx {idx}: {got_phase} != {phase}"
        );
    }

    let (amp, phase) = HarmonicEditor::from_frequency_bin(4, ComplexSample::ZERO);
    assert_eq!(amp, 0.0);
    assert_eq!(phase, 0.0);
}

#[test]
fn exposes_spectrum_input() {
    let editor = HarmonicEditor::new(SRC_ID);

    assert!(editor.inputs().iter().any(|input| {
        input.input_type == Input::Spectrum && input.data_type == DataType::Spectral
    }));
}

fn patterned_source() -> HarmonicEditorConfig {
    let mut config = HarmonicEditorConfig {
        id: SRC_ID,
        bandwidth: 8,
        ..HarmonicEditorConfig::default()
    };

    for channel in 0..NUM_CHANNELS {
        config.amplitudes[channel].fill(0.0);
        config.phases[channel].fill(0.0);
        config.amplitudes[channel][2] = 1.0;
        config.phases[channel][2] = 0.0;
        config.amplitudes[channel][3] = 0.25;
        config.phases[channel][3] = 0.5;
        config.amplitudes[channel][5] = 0.8;
        config.phases[channel][5] = 0.3;
    }

    config
}

fn sampling_engine(capture_input: bool) -> SynthEngine {
    let config = EngineConfig {
        engine: EngineParams::default(),
        modules: vec![
            ModuleConfig::HarmonicEditor(Box::new(patterned_source())),
            ModuleConfig::HarmonicEditor(Box::new(HarmonicEditorConfig {
                id: DST_ID,
                capture_input,
                ..HarmonicEditorConfig::default()
            })),
        ],
        links: vec![LinkConfig::direct(SRC_ID, DST_ID, Input::Spectrum)],
    };

    SynthEngine::try_new(&config, SAMPLE_RATE).expect("valid engine")
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

    let mut left = vec![0.0; 64];
    let mut right = vec![0.0; 64];
    let mut terminated = Vec::new();

    engine.process(64, false, &mut terminated, [&mut left[..], &mut right[..]]);
    (left, right)
}

fn max_abs_diff(a: &[Sample], b: &[Sample]) -> Sample {
    a.iter()
        .zip(b)
        .map(|(x, y)| (x - y).abs())
        .fold(0.0, Sample::max)
}

fn editor_config(engine: &SynthEngine, id: ModuleId) -> HarmonicEditorConfig {
    engine
        .get_config()
        .modules
        .into_iter()
        .find_map(|module| match module {
            ModuleConfig::HarmonicEditor(cfg) if cfg.id == id => Some(*cfg),
            _ => None,
        })
        .expect("harmonic editor")
}

#[test]
fn capture_input_copies_spectrum_on_note() {
    let mut engine = sampling_engine(true);

    play(&mut engine);

    let sampled = editor_config(&engine, DST_ID);
    let source = patterned_source();

    for channel in 0..NUM_CHANNELS {
        for idx in [2, 3, 5] {
            assert!(
                (sampled.amplitudes[channel][idx] - source.amplitudes[channel][idx]).abs() < 1e-4,
                "amp channel {channel} idx {idx}"
            );
            assert!(
                (sampled.phases[channel][idx] - source.phases[channel][idx]).abs() < 1e-4,
                "phase channel {channel} idx {idx}"
            );
        }

        assert_eq!(sampled.amplitudes[channel][1], 0.0);
        assert_eq!(sampled.amplitudes[channel][20], 0.0);
        assert_eq!(sampled.phases[channel][20], 0.0);
    }
}

#[test]
fn capture_input_off_keeps_stored_harmonics() {
    let mut engine = sampling_engine(false);

    play(&mut engine);

    let stored = editor_config(&engine, DST_ID);

    assert!((stored.amplitudes[LEFT_CHANNEL][1] - 1.0).abs() < 1e-6);
    assert!((stored.amplitudes[LEFT_CHANNEL][20] - 1.0).abs() < 1e-6);
    assert!(!stored.capture_input);
}

#[test]
fn capture_input_without_note_keeps_stored_harmonics() {
    let mut engine = sampling_engine(true);
    let mut left = vec![0.0; 64];
    let mut right = vec![0.0; 64];
    let mut terminated = Vec::new();

    engine.process(64, false, &mut terminated, [&mut left[..], &mut right[..]]);

    let stored = editor_config(&engine, DST_ID);

    assert!((stored.amplitudes[LEFT_CHANNEL][1] - 1.0).abs() < 1e-6);
    assert!(stored.capture_input);
}

#[test]
fn sampled_spectrum_matches_direct_source_on_note() {
    const OSC_ID: ModuleId = 3;

    let mut direct = {
        let config = EngineConfig {
            engine: EngineParams::default(),
            modules: vec![
                ModuleConfig::HarmonicEditor(Box::new(patterned_source())),
                ModuleConfig::Oscillator(Box::new(OscillatorConfig {
                    id: OSC_ID,
                    ..OscillatorConfig::default()
                })),
            ],
            links: vec![
                LinkConfig::direct(SRC_ID, OSC_ID, Input::Spectrum),
                LinkConfig::direct(OSC_ID, OUTPUT_MODULE_ID, Input::Audio),
            ],
        };

        SynthEngine::try_new(&config, SAMPLE_RATE).expect("valid engine")
    };

    let mut sampled = {
        let config = EngineConfig {
            engine: EngineParams::default(),
            modules: vec![
                ModuleConfig::HarmonicEditor(Box::new(patterned_source())),
                ModuleConfig::HarmonicEditor(Box::new(HarmonicEditorConfig {
                    id: DST_ID,
                    bandwidth: 8,
                    capture_input: true,
                    ..HarmonicEditorConfig::default()
                })),
                ModuleConfig::Oscillator(Box::new(OscillatorConfig {
                    id: OSC_ID,
                    ..OscillatorConfig::default()
                })),
            ],
            links: vec![
                LinkConfig::direct(SRC_ID, DST_ID, Input::Spectrum),
                LinkConfig::direct(DST_ID, OSC_ID, Input::Spectrum),
                LinkConfig::direct(OSC_ID, OUTPUT_MODULE_ID, Input::Audio),
            ],
        };

        SynthEngine::try_new(&config, SAMPLE_RATE).expect("valid engine")
    };

    let (direct_left, direct_right) = play(&mut direct);
    let (sampled_left, sampled_right) = play(&mut sampled);

    assert!(
        max_abs_diff(&direct_left, &sampled_left) < 1e-4,
        "left channel of the triggered note must use the captured spectrum"
    );
    assert!(
        max_abs_diff(&direct_right, &sampled_right) < 1e-4,
        "right channel of the triggered note must use the captured spectrum"
    );
}
