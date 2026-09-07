use super::{Oscillator, OscillatorConfig, OscillatorUiBridge, SynthModule};
use crate::synth_engine::{
    EngineConfig, EngineParams, Input, LinkConfig, MAX_BLOCK_SIZE, ModuleConfig, ModuleHandle,
    ModuleId, Note, Sample, SynthEngine,
    harmonic_editor::HarmonicEditorConfig,
    routing::{LEFT_CHANNEL, OUTPUT_MODULE_ID, RIGHT_CHANNEL, VoiceEvent},
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

fn process_samples(engine: &mut SynthEngine, samples: usize) {
    let mut remaining = samples;

    while remaining > 0 {
        let n = remaining.min(MAX_BLOCK_SIZE);
        process_block(engine, n);
        remaining -= n;
    }
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

fn reset_event(prev_pitch: Option<Sample>, pitch: Sample) -> VoiceEvent {
    VoiceEvent::Reset {
        voice_idx: 0,
        replaced_voice_idx: None,
        prev_pitch,
        pitch,
        velocity: 1.0,
        offset: 0,
    }
}

fn update_event(pitch: Sample) -> VoiceEvent {
    VoiceEvent::Update {
        voice_idx: 0,
        pitch,
        velocity: 1.0,
        offset: 0,
    }
}

fn voice_pitch(osc: &Oscillator) -> Sample {
    osc.voices[LEFT_CHANNEL][0].pitch
}

fn voice_glide_from(osc: &Oscillator) -> Option<Sample> {
    osc.voices[LEFT_CHANNEL][0]
        .glide
        .as_ref()
        .map(|glide| glide.pitch_from)
}

fn any_glide_current_pitch(osc: &Oscillator) -> Option<Sample> {
    osc.voices[LEFT_CHANNEL]
        .iter()
        .find_map(|voice| voice.glide.as_ref().map(|glide| glide.current_pitch))
}

fn make_osc_engine(osc: OscillatorConfig) -> SynthEngine {
    let config = EngineConfig {
        engine: EngineParams::default(),
        modules: vec![
            ModuleConfig::HarmonicEditor(Box::new(HarmonicEditorConfig {
                id: HARMONIC_EDITOR_ID,
                ..HarmonicEditorConfig::default()
            })),
            ModuleConfig::Oscillator(Box::new(osc)),
        ],
        links: vec![
            LinkConfig::direct(HARMONIC_EDITOR_ID, OSCILLATOR_ID, Input::Spectrum),
            LinkConfig::direct(OSCILLATOR_ID, OUTPUT_MODULE_ID, Input::Audio),
        ],
    };

    SynthEngine::try_new(&config, SAMPLE_RATE).expect("valid engine config")
}

fn oscillator(engine: &SynthEngine) -> &Oscillator {
    match engine.get_module(OSCILLATOR_ID) {
        Some(ModuleHandle::Oscillator(osc)) => osc,
        _ => panic!("expected oscillator module"),
    }
}

fn note(midi: u8) -> Note {
    Note {
        channel: 0,
        note: midi,
        velocity: 1.0,
        host_id: None,
    }
}

#[test]
fn glide_always_defaults_to_true() {
    assert!(OscillatorConfig::default().glide_always);
    assert!(Oscillator::new(1).get_config().glide_always);
}

#[test]
fn glide_per_octave_defaults_to_false() {
    assert!(!OscillatorConfig::default().glide_per_octave);
    assert!(!Oscillator::new(1).get_config().glide_per_octave);
}

#[test]
fn glide_options_default_when_missing_from_json() {
    let mut json = serde_json::to_value(OscillatorConfig::default()).unwrap();
    let obj = json.as_object_mut().unwrap();
    obj.remove("glide_always");
    obj.remove("glide_per_octave");

    let config: OscillatorConfig = serde_json::from_value(json).unwrap();

    assert!(config.glide_always);
    assert!(!config.glide_per_octave);
}

#[test]
fn glide_options_round_trip_through_config() {
    let osc = Oscillator::from_config(&OscillatorConfig {
        id: 1,
        glide_always: false,
        glide_per_octave: true,
        ..OscillatorConfig::default()
    });

    let config = osc.get_config();

    assert!(!config.glide_always);
    assert!(config.glide_per_octave);
}

#[test]
fn set_glide_options_update_config() {
    let mut osc = Oscillator::new(1);

    osc.set_glide_always(false);
    osc.set_glide_per_octave(true);

    assert!(!osc.get_config().glide_always);
    assert!(osc.get_config().glide_per_octave);
}

#[test]
fn ui_events_set_glide_options() {
    let mut osc = Oscillator::new(1);
    let mut bridge = OscillatorUiBridge::try_new(&mut osc).expect("ui end");

    bridge.set_glide_always(false);
    bridge.set_glide_per_octave(true);
    osc.process_ui_events();

    assert!(!bridge.config().glide_always);
    assert!(bridge.config().glide_per_octave);
    assert!(!osc.get_config().glide_always);
    assert!(osc.get_config().glide_per_octave);
}

#[test]
fn glide_always_starts_glide_on_reset_with_prev_pitch() {
    let mut osc = Oscillator::new(1);

    osc.process_events(&[reset_event(Some(0.0), 1.0)]);

    assert_eq!(voice_pitch(&osc), 1.0);
    assert_eq!(voice_glide_from(&osc), Some(0.0));
}

#[test]
fn glide_always_false_skips_glide_on_reset() {
    let mut osc = Oscillator::from_config(&OscillatorConfig {
        id: 1,
        glide_always: false,
        ..OscillatorConfig::default()
    });

    osc.process_events(&[reset_event(Some(0.0), 1.0)]);

    assert_eq!(voice_pitch(&osc), 1.0);
    assert_eq!(voice_glide_from(&osc), None);
}

#[test]
fn glide_always_false_still_glides_on_legato_update() {
    let mut osc = Oscillator::from_config(&OscillatorConfig {
        id: 1,
        glide_always: false,
        ..OscillatorConfig::default()
    });

    osc.process_events(&[reset_event(None, 0.0), update_event(1.0)]);

    assert_eq!(voice_pitch(&osc), 1.0);
    assert_eq!(voice_glide_from(&osc), Some(0.0));
}

#[test]
fn glide_per_octave_scales_duration_by_pitch_interval() {
    const GLIDE_TIME: Sample = 0.1;
    const PROCESS_TIME: Sample = 0.05;
    const FROM_NOTE: u8 = 60;
    const TWO_OCTAVES_NOTE: u8 = 84;
    let samples = (PROCESS_TIME * SAMPLE_RATE) as usize;

    let process_interval = |glide_per_octave: bool| {
        let mut engine = make_osc_engine(OscillatorConfig {
            id: OSCILLATOR_ID,
            glide: GLIDE_TIME.into(),
            glide_per_octave,
            ..OscillatorConfig::default()
        });

        engine.handle_note_on(note(FROM_NOTE), 0);
        process_samples(&mut engine, MAX_BLOCK_SIZE);

        engine.handle_note_on(note(TWO_OCTAVES_NOTE), 0);
        process_samples(&mut engine, samples);

        any_glide_current_pitch(oscillator(&engine)).expect("glide in progress")
    };

    let from_pitch = crate::utils::note_to_pitch(FROM_NOTE as Sample);
    let to_pitch = crate::utils::note_to_pitch(TWO_OCTAVES_NOTE as Sample);
    let pitch_diff = to_pitch - from_pitch;
    let elapsed_frac = PROCESS_TIME / GLIDE_TIME;

    let without_per_octave = process_interval(false);
    let with_per_octave = process_interval(true);

    assert!((without_per_octave - (from_pitch + pitch_diff * elapsed_frac)).abs() < 1e-3);
    assert!(
        (with_per_octave - (from_pitch + pitch_diff * elapsed_frac / pitch_diff.abs())).abs()
            < 1e-3
    );
}
