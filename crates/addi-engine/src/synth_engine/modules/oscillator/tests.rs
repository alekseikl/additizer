use realfft::RealFftPlanner;

use super::{
    DFT_BUFFER_SIZE, IfftPlanners, Oscillator, OscillatorConfig, WAVEFORM_BUFFER_SIZE,
    WAVEFORM_PAD_LEFT, WAVEFORM_SIZE, Waveform, WaveformBuffer, WaveformSize,
};
use crate::synth_engine::{
    ComplexSample, EngineConfig, EngineParams, Input, LinkConfig, ModuleConfig, ModuleId, Note,
    OUTPUT_MODULE_ID, Sample, SynthEngine, harmonic_editor::HarmonicEditorConfig,
    routing::RIGHT_CHANNEL,
};

const SAMPLE_RATE: Sample = 48_000.0;
const HARMONIC_EDITOR_ID: ModuleId = 1;
const OSCILLATOR_ID: ModuleId = 2;

fn make_engine(he: HarmonicEditorConfig, mono_spectrum: bool) -> SynthEngine {
    make_engine_with(
        he,
        OscillatorConfig {
            id: OSCILLATOR_ID,
            mono_spectrum,
            ..OscillatorConfig::default()
        },
    )
}

fn make_engine_with(he: HarmonicEditorConfig, osc: OscillatorConfig) -> SynthEngine {
    let config = EngineConfig {
        engine: EngineParams::default(),
        modules: vec![
            ModuleConfig::HarmonicEditor(Box::new(he)),
            ModuleConfig::Oscillator(Box::new(osc)),
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

    he.amplitudes[RIGHT_CHANNEL].fill(0.0);

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
    assert!(!<Oscillator>::new(1).get_config().mono_spectrum);
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
    let osc = <Oscillator>::from_config(&OscillatorConfig {
        id: 1,
        mono_spectrum: true,
        ..OscillatorConfig::default()
    });

    assert!(osc.get_config().mono_spectrum);
}

#[test]
fn set_mono_spectrum_updates_config() {
    let mut osc = <Oscillator>::new(1);

    osc.set_mono_spectrum(true);

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

fn default_harmonics() -> HarmonicEditorConfig {
    HarmonicEditorConfig {
        id: HARMONIC_EDITOR_ID,
        ..HarmonicEditorConfig::default()
    }
}

/// With no detune and no per-voice phase/gain offsets, `n` unison voices are
/// identical copies, so after the `1 / sqrt(sum gain²)` unison normalization
/// the output must equal `sqrt(n)` times the single-voice output. Covers both
/// the full SIMD chunks and the partial-chunk remainder of the render loop.
#[test]
fn coherent_unison_scales_single_voice_by_sqrt_n() {
    let coherent = |unison_voices: usize| OscillatorConfig {
        id: OSCILLATOR_ID,
        unison_voices,
        detune: 0.0.into(),
        ..OscillatorConfig::default()
    };

    let mut single = make_engine_with(default_harmonics(), coherent(1));
    let (single_left, single_right) = play(&mut single);
    assert!(rms(&single_left) > 1e-3);

    for unison_voices in [2, 3, 4, 5, 7, 8, 13, 16] {
        let mut engine = make_engine_with(default_harmonics(), coherent(unison_voices));
        let (left, right) = play(&mut engine);
        let scale = (unison_voices as Sample).sqrt();

        for (expected, actual) in [(&single_left, &left), (&single_right, &right)] {
            let scaled: Vec<Sample> = expected.iter().map(|s| s * scale).collect();

            assert!(
                max_abs_diff(&scaled, actual) < 1e-4,
                "unison {unison_voices}: max diff {}",
                max_abs_diff(&scaled, actual)
            );
        }
    }
}

fn wave_cutoff(frequency: Sample, sample_rate: Sample, spectrum_len: usize) -> usize {
    let max_frequency = 0.5 * sample_rate;
    ((max_frequency / frequency).floor() as usize + 1).min(spectrum_len)
}

fn reference_full_wave(spectrum: &[ComplexSample], cutoff: usize) -> Box<WaveformBuffer> {
    let fft = RealFftPlanner::<Sample>::new().plan_fft_inverse(WAVEFORM_SIZE);
    let mut dft = [ComplexSample::ZERO; DFT_BUFFER_SIZE];
    let mut scratch = [ComplexSample::ZERO; DFT_BUFFER_SIZE];
    let mut wave = Box::new([0.0; WAVEFORM_BUFFER_SIZE]);
    let complex_len = fft.complex_len();
    let copy_len = cutoff.min(complex_len).min(spectrum.len());

    dft[..copy_len].copy_from_slice(&spectrum[..copy_len]);
    fft.process_with_scratch(
        &mut dft[..complex_len],
        Oscillator::<super::stub::Links>::waveform_body_mut(&mut wave, WaveformSize::Full),
        &mut scratch,
    )
    .expect("reference ifft");

    wave
}

fn build_test_wave(frequency: Sample, spectrum: &[ComplexSample]) -> Waveform {
    let ifft = IfftPlanners::new();
    let mut dft = [ComplexSample::ZERO; DFT_BUFFER_SIZE];
    let mut scratch = [ComplexSample::ZERO; DFT_BUFFER_SIZE];
    let mut wave = Waveform::new();

    Oscillator::<super::stub::Links>::build_wave(
        &ifft,
        frequency,
        SAMPLE_RATE,
        spectrum,
        &mut dft,
        &mut scratch,
        &mut wave,
    );

    wave
}

fn body(wave: &WaveformBuffer, len: usize) -> &[Sample] {
    &wave[WAVEFORM_PAD_LEFT..WAVEFORM_PAD_LEFT + len]
}

fn assert_wave_matches_full(frequency: Sample, spectrum: &[ComplexSample], size: WaveformSize) {
    let cutoff = wave_cutoff(frequency, SAMPLE_RATE, spectrum.len());
    let reference = reference_full_wave(spectrum, cutoff);
    let wave = build_test_wave(frequency, spectrum);
    let len = size.len();
    let stride = WAVEFORM_SIZE / len;
    let actual = body(&wave.samples, len);
    let expected = body(&reference, WAVEFORM_SIZE);
    let peak = expected.iter().fold(0.0_f32, |acc, s| acc.max(s.abs()));
    let max_diff = actual
        .iter()
        .enumerate()
        .map(|(i, sample)| (sample - expected[i * stride]).abs())
        .fold(0.0_f32, Sample::max);
    let last = WAVEFORM_PAD_LEFT + len - 1;

    assert_eq!(wave.size, size);
    assert!(peak > 1.0, "frequency {frequency}: reference peak {peak}");
    assert!(
        max_diff < peak * 1e-4,
        "frequency {frequency}: max diff {max_diff}, peak {peak}"
    );
    assert_eq!(wave.samples[0], wave.samples[last]);
    assert_eq!(wave.samples[last + 1], wave.samples[WAVEFORM_PAD_LEFT]);
    assert_eq!(wave.samples[last + 2], wave.samples[WAVEFORM_PAD_LEFT + 1]);
}

#[test]
fn build_wave_uses_half_resolution_for_high_notes() {
    let mut spectrum = vec![ComplexSample::ZERO; DFT_BUFFER_SIZE];
    spectrum[1] = ComplexSample::new(0.8, 0.1);
    spectrum[80] = ComplexSample::new(0.4, -0.3);

    assert_wave_matches_full(200.0, &spectrum, WaveformSize::Half);
}

#[test]
fn build_wave_uses_half_resolution_for_mid_notes() {
    let mut spectrum = vec![ComplexSample::ZERO; DFT_BUFFER_SIZE];
    spectrum[1] = ComplexSample::new(0.8, 0.1);
    spectrum[200] = ComplexSample::new(0.35, 0.25);

    assert_wave_matches_full(80.0, &spectrum, WaveformSize::Half);
}

#[test]
fn build_wave_keeps_full_resolution_when_cutoff_reaches_half_nyquist() {
    let mut spectrum = vec![ComplexSample::ZERO; DFT_BUFFER_SIZE];
    spectrum[1] = ComplexSample::new(0.8, 0.0);
    spectrum[200] = ComplexSample::new(0.2, 0.1);

    let cutoff = wave_cutoff(46.8, SAMPLE_RATE, spectrum.len());
    assert_eq!(cutoff, WaveformSize::Half.len() / 2 + 1);

    assert_wave_matches_full(46.8, &spectrum, WaveformSize::Full);
}

#[test]
fn build_wave_keeps_full_resolution_past_half_table() {
    let mut spectrum = vec![ComplexSample::ZERO; DFT_BUFFER_SIZE];
    spectrum[1] = ComplexSample::new(0.8, 0.0);
    spectrum[520] = ComplexSample::new(0.05, 0.2);

    let cutoff = wave_cutoff(46.0, SAMPLE_RATE, spectrum.len());
    assert!(cutoff > WaveformSize::Half.len() / 2 + 1);

    assert_wave_matches_full(46.0, &spectrum, WaveformSize::Full);
}
