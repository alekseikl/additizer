use realfft::RealFftPlanner;

use super::{
    DFT_BUFFER_SIZE, HALF_WAVEFORM_BITS, IfftPlanners, Interpolated, MAX_UNISON_VOICES, Oscillator,
    OscillatorConfig, OscillatorLinks, UnisonStyle, UnisonVoice, VoiceRenderCtx, WAVEFORM_BITS,
    WAVEFORM_BUFFER_SIZE, WAVEFORM_PAD_LEFT, WAVEFORM_SIZE, Waveform, WaveformBuffer, WaveformSize,
    lanes::{UNISON_LANES, UnisonLaneParams},
};
use crate::synth_engine::{
    ComplexSample, EngineConfig, EngineParams, Input, LinkConfig, MAX_VOICES, ModuleConfig,
    ModuleId, NUM_CHANNELS, Note, OUTPUT_MODULE_ID, Sample, SynthEngine,
    coeffs::catmull_rom_from_powers,
    harmonic_editor::HarmonicEditorConfig,
    phase::Phase,
    routing::{InputSlot, LEFT_CHANNEL, RIGHT_CHANNEL},
};

impl<L: OscillatorLinks> Oscillator<L> {
    pub(crate) fn spectrum_slot(&self) -> Option<usize> {
        self.inputs.spectrum
    }

    pub(crate) fn pitch_slot(&self) -> Option<usize> {
        self.inputs.pitch
    }

    pub(crate) fn detune_slots(&self) -> &[InputSlot] {
        &self.inputs.detune.slots
    }
}

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
fn phase_random_stereo_defaults_to_shared_phases() {
    assert!(!OscillatorConfig::default().phase_random_stereo);
    assert!(!<Oscillator>::new(1).get_config().phase_random_stereo);
}

#[test]
fn phase_random_stereo_defaults_when_missing_from_json() {
    let mut json = serde_json::to_value(OscillatorConfig::default()).unwrap();
    json.as_object_mut().unwrap().remove("phase_random_stereo");

    let config: OscillatorConfig = serde_json::from_value(json).unwrap();

    assert!(!config.phase_random_stereo);
}

#[test]
fn shared_phase_random_matches_both_channels() {
    let mut engine = make_engine_with(
        default_harmonics(),
        OscillatorConfig {
            id: OSCILLATOR_ID,
            unison_voices: 2,
            phase_random: 1.0,
            phase_random_stereo: false,
            ..OscillatorConfig::default()
        },
    );
    let (left, right) = play(&mut engine);

    assert!(rms(&left) > 1e-3);
    assert!(max_abs_diff(&left, &right) < 1e-5);
}

#[test]
fn stereo_phase_random_gives_each_channel_its_own_phases() {
    let mut engine = make_engine_with(
        default_harmonics(),
        OscillatorConfig {
            id: OSCILLATOR_ID,
            unison_voices: 2,
            phase_random: 1.0,
            phase_random_stereo: true,
            ..OscillatorConfig::default()
        },
    );
    let (left, right) = play(&mut engine);

    assert!(rms(&left) > 1e-3);
    assert!(rms(&right) > 1e-3);
    assert!(max_abs_diff(&left, &right) > 1e-3);
}

#[test]
fn unison_style_defaults_to_custom_and_full_stereo() {
    let config = OscillatorConfig::default();

    assert_eq!(config.unison_style, UnisonStyle::Custom);
    assert_eq!(config.unison_stereo, 1.0);
    assert_eq!(
        <Oscillator>::new(1).get_config().unison_style,
        UnisonStyle::Custom
    );
}

#[test]
fn unison_style_defaults_when_missing_from_json() {
    let mut json = serde_json::to_value(OscillatorConfig::default()).unwrap();
    let object = json.as_object_mut().unwrap();
    object.remove("unison_style");
    object.remove("unison_stereo");

    let config: OscillatorConfig = serde_json::from_value(json).unwrap();

    assert_eq!(config.unison_style, UnisonStyle::Custom);
    assert_eq!(config.unison_stereo, 1.0);
}

fn approx_eq(actual: Sample, expected: Sample) {
    assert!((actual - expected).abs() < 1e-5, "{actual} != {expected}");
}

#[test]
fn style1_rates_follow_paired_spread() {
    let unison = 5;
    let detune = 1.0;

    // Positions are -1, -1/2, 0, +1/2, +1. One octave of detune.
    approx_eq(<Oscillator>::style1_rate(0, unison, detune, 0.0), 0.5);
    approx_eq(
        <Oscillator>::style1_rate(1, unison, detune, 0.0),
        (-0.5f32).exp2(),
    );
    approx_eq(<Oscillator>::style1_rate(2, unison, detune, 0.0), 1.0);
    approx_eq(
        <Oscillator>::style1_rate(3, unison, detune, 0.0),
        0.5f32.exp2(),
    );
    approx_eq(<Oscillator>::style1_rate(4, unison, detune, 0.0), 2.0);

    // Even count has no center: ±1/3 and ±1.
    approx_eq(<Oscillator>::style1_rate(0, 4, detune, 0.0), 0.5);
    approx_eq(
        <Oscillator>::style1_rate(1, 4, detune, 0.0),
        (-1.0f32 / 3.0).exp2(),
    );
    approx_eq(
        <Oscillator>::style1_rate(2, 4, detune, 0.0),
        (1.0f32 / 3.0).exp2(),
    );
    approx_eq(<Oscillator>::style1_rate(3, 4, detune, 0.0), 2.0);
}

#[test]
fn style1_voice_gain_matches_set_amplitude_and_stereo_blend() {
    let unison = 5;
    let blend = 1.0;
    // center 0.4, detuned 0.6, two detuned pairs: 1/sqrt(0.16 + 0.36*2)
    let scale = (0.88f32).sqrt().recip();
    let center_amp = 0.4 * scale;
    let detuned_amp = 0.6 * scale;

    // Full stereo. Left keeps -1 and +1/2, right keeps -1/2 and +1, both keep the center.
    let on_left = [true, false, true, true, false];
    let on_right = [false, true, true, false, true];
    for idx in 0..unison {
        let amp = if idx == 2 { center_amp } else { detuned_amp };
        let left = <Oscillator>::style1_voice_gain(idx, unison, blend, 1.0, LEFT_CHANNEL);
        let right = <Oscillator>::style1_voice_gain(idx, unison, blend, 1.0, RIGHT_CHANNEL);
        approx_eq(left, if on_left[idx] { amp } else { 0.0 });
        approx_eq(right, if on_right[idx] { amp } else { 0.0 });
    }

    // Spread 0 mixes both channels at 1/√2. The center exists on both, so it adds.
    let half = std::f32::consts::FRAC_1_SQRT_2;
    for idx in 0..unison {
        let left = <Oscillator>::style1_voice_gain(idx, unison, blend, 0.0, LEFT_CHANNEL);
        let right = <Oscillator>::style1_voice_gain(idx, unison, blend, 0.0, RIGHT_CHANNEL);
        let expected = if idx == 2 {
            center_amp * std::f32::consts::SQRT_2
        } else {
            detuned_amp * half
        };
        approx_eq(left, expected);
        approx_eq(right, expected);
    }
    let center_mono = <Oscillator>::style1_voice_gain(2, unison, blend, 0.0, LEFT_CHANNEL);
    let edge_mono = <Oscillator>::style1_voice_gain(0, unison, blend, 0.0, LEFT_CHANNEL);
    approx_eq(center_mono / edge_mono, 4.0 / 3.0);

    // Blend 0 drops every detuned voice and leaves the center at unity.
    for idx in 0..unison {
        let gain = <Oscillator>::style1_voice_gain(idx, unison, 0.0, 1.0, LEFT_CHANNEL);
        approx_eq(gain, if idx == 2 { 1.0 } else { 0.0 });
    }
}

#[test]
fn style1_even_unison_uses_center_level_for_the_inner_pair() {
    // Four voices: ±1/3 use the center level, ±1 use the detuned level.
    // At full stereo the left channel gets -1/3 and +1.
    let scale = (0.52f32).sqrt().recip();
    let center_amp = 0.4 * scale;
    let detuned_amp = 0.6 * scale;

    approx_eq(
        <Oscillator>::style1_voice_gain(0, 4, 1.0, 1.0, LEFT_CHANNEL),
        0.0,
    );
    approx_eq(
        <Oscillator>::style1_voice_gain(1, 4, 1.0, 1.0, LEFT_CHANNEL),
        center_amp,
    );
    approx_eq(
        <Oscillator>::style1_voice_gain(2, 4, 1.0, 1.0, LEFT_CHANNEL),
        0.0,
    );
    approx_eq(
        <Oscillator>::style1_voice_gain(3, 4, 1.0, 1.0, LEFT_CHANNEL),
        detuned_amp,
    );
    assert!(center_amp < detuned_amp);

    // Two voices: hard opposite detune, unity level, no blend.
    approx_eq(
        <Oscillator>::style1_voice_gain(0, 2, 1.0, 1.0, LEFT_CHANNEL),
        1.0,
    );
    approx_eq(
        <Oscillator>::style1_voice_gain(1, 2, 1.0, 1.0, LEFT_CHANNEL),
        0.0,
    );
    approx_eq(
        <Oscillator>::style1_voice_gain(0, 2, 0.0, 1.0, RIGHT_CHANNEL),
        0.0,
    );
    approx_eq(
        <Oscillator>::style1_voice_gain(1, 2, 0.0, 1.0, RIGHT_CHANNEL),
        1.0,
    );
    let mono = std::f32::consts::FRAC_1_SQRT_2;
    approx_eq(
        <Oscillator>::style1_voice_gain(0, 2, 1.0, 0.0, LEFT_CHANNEL),
        mono,
    );
    approx_eq(
        <Oscillator>::style1_voice_gain(1, 2, 1.0, 0.0, RIGHT_CHANNEL),
        mono,
    );
}

fn zero_level_unison(style: UnisonStyle, stereo: Sample) -> OscillatorConfig {
    let mut config = OscillatorConfig {
        id: OSCILLATOR_ID,
        unison_voices: 5,
        unison_style: style,
        unison_stereo: stereo,
        gains_blend: 1.0.into(),
        detune: 0.25.into(),
        ..OscillatorConfig::default()
    };

    for (idx, voice) in config.unison.iter_mut().enumerate() {
        voice.initial_phase = (idx as Sample * 0.17).into();
        voice.gain = 0.0.into();
        voice.gain_to = 0.0.into();
    }

    config
}

#[test]
fn custom_unison_follows_hand_levels() {
    let mut engine = make_engine_with(
        default_harmonics(),
        zero_level_unison(UnisonStyle::Custom, 1.0),
    );
    let (left, right) = play(&mut engine);

    assert!(rms(&left) < 1e-5);
    assert!(rms(&right) < 1e-5);
}

#[test]
fn style1_unison_shapes_levels_and_spreads_channels() {
    let mut mono = make_engine_with(
        default_harmonics(),
        zero_level_unison(UnisonStyle::Style1, 0.0),
    );
    let (mono_left, mono_right) = play(&mut mono);

    assert!(rms(&mono_left) > 1e-3);
    assert!(max_abs_diff(&mono_left, &mono_right) < 1e-4);

    let mut wide = make_engine_with(
        default_harmonics(),
        zero_level_unison(UnisonStyle::Style1, 1.0),
    );
    let (wide_left, wide_right) = play(&mut wide);

    assert!(rms(&wide_left) > 1e-3);
    assert!(rms(&wide_right) > 1e-3);
    assert!(max_abs_diff(&wide_left, &wide_right) > 1e-3);
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

const RENDER_SENTINEL: Sample = 50.0;
const FREQ_PHASE_MULT: Sample = 16.0;
const UNISON_GAIN_FROM: Sample = 0.55;
const UNISON_GAIN_TO: Sample = 0.85;
const DECOY_PHASE: u32 = 0x1111_1111;

struct RenderCase {
    name: &'static str,
    unison: usize,
    from_size: WaveformSize,
    to_size: WaveformSize,
    samples: usize,
    start: usize,
    end: usize,
    channel: usize,
    voice: usize,
    wave_channel: usize,
}

fn render_case(
    name: &'static str,
    unison: usize,
    from_size: WaveformSize,
    to_size: WaveformSize,
) -> RenderCase {
    RenderCase {
        name,
        unison,
        from_size,
        to_size,
        samples: 32,
        start: 0,
        end: 32,
        channel: 0,
        voice: 0,
        wave_channel: 0,
    }
}

struct SampleControls {
    phase_inc: Sample,
    phase_shift: Sample,
    frequency_shift: Sample,
}

fn sample_controls(index: usize) -> SampleControls {
    let n = index as Sample;

    SampleControls {
        phase_inc: 262_144.0 + n * 128.0,
        phase_shift: 0.02 + 0.001 * n,
        frequency_shift: 64.0 + 2.0 * n,
    }
}

fn test_phase(index: usize) -> Phase {
    if index == 0 {
        // Last full-table bin, so the first read uses the wrap padding.
        Phase::from_bits(0xffff_0000)
    } else {
        Phase::from_bits(
            0x1000_0000u32
                .wrapping_mul(index as u32)
                .wrapping_add(0x0008_0000),
        )
    }
}

fn test_unison_voice(index: usize) -> UnisonVoice {
    let i = index as Sample;

    UnisonVoice {
        rate: Interpolated {
            from: 0.85 + 0.03 * i,
            to: 1.15 - 0.02 * i,
        },
        phase_shift: Interpolated {
            from: -0.2 + 0.02 * i,
            to: 0.15 - 0.015 * i,
        },
        gain: Interpolated {
            from: 0.35 - 0.08 * i,
            to: -0.25 + 0.06 * i,
        },
    }
}

fn initial_phases() -> [Phase; MAX_UNISON_VOICES] {
    std::array::from_fn(test_phase)
}

fn initial_voices() -> [UnisonVoice; MAX_UNISON_VOICES] {
    std::array::from_fn(test_unison_voice)
}

/// Same wrap as `PhaseX4::wrap_normalized`: one cycle, half away from zero.
fn wrap_unit(phase: Sample) -> Sample {
    phase - phase.round()
}

fn interpolated_sample(
    wave: &WaveformBuffer,
    phase: Phase,
    size: WaveformSize,
    gain: Sample,
) -> Sample {
    let (idx, t) = match size {
        WaveformSize::Full => (
            phase.wave_index::<WAVEFORM_BITS>(),
            phase.wave_index_fraction::<WAVEFORM_BITS>(),
        ),
        WaveformSize::Half => (
            phase.wave_index::<HALF_WAVEFORM_BITS>(),
            phase.wave_index_fraction::<HALF_WAVEFORM_BITS>(),
        ),
    };
    let gt = gain * t;
    let gt2 = gt * t;
    let gt3 = gt2 * t;
    let weights = catmull_rom_from_powers(gain, gt, gt2, gt3).to_array();

    weights
        .iter()
        .zip(&wave[idx..idx + 4])
        .map(|(weight, sample)| weight * sample)
        .sum()
}

fn patterned_wave(size: WaveformSize, seed: Sample) -> Box<WaveformBuffer> {
    let mut samples = Box::new([RENDER_SENTINEL; WAVEFORM_BUFFER_SIZE]);
    let len = size.len();

    for i in 0..len {
        let n = (i + 1) as Sample;
        samples[WAVEFORM_PAD_LEFT + i] = (seed * n).sin() * 0.5 + (n * 0.17 + seed).cos() * 0.25;
    }

    Oscillator::<super::stub::Links>::wrap_waveform(&mut samples, size);
    samples
}

/// Scalar Catmull-Rom render of one block. `buff_t` follows the audio-thread
/// recurrence (`start / n`, then `+= 1/n`) so phase truncation matches.
fn reference_render(
    case: &RenderCase,
    wave_from: &WaveformBuffer,
    wave_to: &WaveformBuffer,
) -> (Vec<Sample>, [Phase; MAX_UNISON_VOICES]) {
    let voices = initial_voices();
    let mut phases = initial_phases();
    let mut output = vec![RENDER_SENTINEL; case.samples];
    let buff_t_inc = (case.samples as Sample).recip();
    let mut buff_t = case.start as Sample * buff_t_inc;
    let full = case.unison / UNISON_LANES;
    let rem = case.unison % UNISON_LANES;

    for (index, sample) in output
        .iter_mut()
        .enumerate()
        .take(case.end)
        .skip(case.start)
    {
        let controls = sample_controls(index);
        let global_shift = Phase::from_normalized(controls.phase_shift);
        let phase_inc = controls.phase_inc + controls.frequency_shift * FREQ_PHASE_MULT;
        let mut acc_from = 0.0;
        let mut acc_to = 0.0;

        let mut advance_chunk = |chunk: usize, lanes: usize| {
            for lane in 0..UNISON_LANES {
                let voice_idx = chunk * UNISON_LANES + lane;
                let (rate_from, rate_delta, phase_from, phase_delta, gain_from, gain_delta) =
                    if lane < lanes {
                        let voice = &voices[voice_idx];
                        let phase_from = wrap_unit(voice.phase_shift.from);
                        let phase_to = wrap_unit(voice.phase_shift.to);

                        let gain_from = voice.gain.from * UNISON_GAIN_FROM;
                        let gain_to = voice.gain.to * UNISON_GAIN_TO;

                        (
                            voice.rate.from,
                            voice.rate.to - voice.rate.from,
                            phase_from,
                            phase_to - phase_from,
                            gain_from,
                            gain_to - gain_from,
                        )
                    } else {
                        (0.0, 0.0, 0.0, 0.0, 0.0, 0.0)
                    };

                let rate = rate_delta.mul_add(buff_t, rate_from);
                let unison_shift = phase_delta.mul_add(buff_t, phase_from);
                let gain = gain_delta.mul_add(buff_t, gain_from);
                let read = phases[voice_idx] + global_shift + Phase::from_normalized(unison_shift);

                if lane < lanes {
                    acc_from += interpolated_sample(wave_from, read, case.from_size, gain);
                    acc_to += interpolated_sample(wave_to, read, case.to_size, gain);
                }

                let inc = rate * phase_inc;
                phases[voice_idx] += inc;
            }
        };

        for chunk in 0..full {
            advance_chunk(chunk, UNISON_LANES);
        }
        if rem > 0 {
            advance_chunk(full, rem);
        }

        *sample = (acc_to - acc_from).mul_add(buff_t, acc_from);
        buff_t += buff_t_inc;
    }

    (output, phases)
}

fn decoy_unison_voice() -> UnisonVoice {
    UnisonVoice {
        rate: Interpolated { from: 2.0, to: 2.0 },
        phase_shift: Interpolated { from: 0.3, to: 0.3 },
        gain: Interpolated { from: 4.0, to: 4.0 },
    }
}

fn store_lane_params(osc: &mut Oscillator, voices: &[UnisonVoice], unison: usize) {
    let full = unison / UNISON_LANES;
    let rem = unison % UNISON_LANES;

    for chunk in 0..full {
        let start = chunk * UNISON_LANES;
        osc.lane_params[chunk] = UnisonLaneParams::from_voices(
            &voices[start..start + UNISON_LANES],
            UNISON_GAIN_FROM,
            UNISON_GAIN_TO,
        );
    }

    if rem > 0 {
        let start = full * UNISON_LANES;
        osc.lane_params[full] = UnisonLaneParams::from_voices(
            &voices[start..start + rem],
            UNISON_GAIN_FROM,
            UNISON_GAIN_TO,
        );
    }
}

fn load_case(
    osc: &mut Oscillator,
    case: &RenderCase,
    wave_from: &WaveformBuffer,
    wave_to: &WaveformBuffer,
) {
    let decoy = patterned_wave(WaveformSize::Full, 9.5);

    for channel in 0..NUM_CHANNELS {
        for voice in 0..MAX_VOICES {
            osc.voice_buffers.at_mut(channel, voice).wave.samples = decoy.clone();
            osc.voice_buffers.at_mut(channel, voice).wave.size = WaveformSize::Full;

            let slot = osc.voices.at_mut(channel, voice);
            slot.phases = [Phase::from_bits(DECOY_PHASE); MAX_UNISON_VOICES];
            slot.unison = std::array::from_fn(|_| decoy_unison_voice());
            slot.unison_gain = Interpolated { from: 4.0, to: 4.0 };
        }
    }

    *osc.voice_buffers
        .at_mut(case.wave_channel, case.voice)
        .wave
        .samples = *wave_from;
    osc.voice_buffers
        .at_mut(case.wave_channel, case.voice)
        .wave
        .size = case.from_size;
    *osc.buffers.tmp_wave.samples = *wave_to;
    osc.buffers.tmp_wave.size = case.to_size;

    let voices = initial_voices();
    store_lane_params(osc, &voices, case.unison);

    let slot = osc.voices.at_mut(case.channel, case.voice);
    slot.phases = initial_phases();
    slot.unison = voices;
    slot.unison_gain = Interpolated {
        from: UNISON_GAIN_FROM,
        to: UNISON_GAIN_TO,
    };
    osc.params.unison = case.unison;

    osc.buffers.phase_inc.fill(0.0);
    osc.buffers.phase_shift.fill(0.0);

    for index in 0..case.samples {
        let controls = sample_controls(index);
        osc.buffers.phase_inc[index] =
            controls.phase_inc + controls.frequency_shift * FREQ_PHASE_MULT;
        osc.buffers.phase_shift[index] = controls.phase_shift;
    }
}

fn render_ctx(case: &RenderCase) -> VoiceRenderCtx {
    VoiceRenderCtx {
        channel_idx: case.channel,
        voice_idx: case.voice,
        wave_channel: case.wave_channel,
        samples: case.samples,
    }
}

fn assert_samples_close(name: &str, expected: &[Sample], actual: &[Sample]) {
    let mut max_diff = 0.0;
    let mut at = 0;

    for (index, (expected, actual)) in expected.iter().zip(actual).enumerate() {
        let diff = (expected - actual).abs();

        if diff > max_diff {
            max_diff = diff;
            at = index;
        }
    }

    assert!(
        max_diff < 1e-4,
        "{name}: max diff {max_diff} at sample {at} (expected {}, actual {})",
        expected[at],
        actual[at]
    );
}

fn assert_phases(
    name: &str,
    expected: &[Phase; MAX_UNISON_VOICES],
    actual: &[Phase; MAX_UNISON_VOICES],
) {
    for (index, (expected, actual)) in expected.iter().zip(actual).enumerate() {
        assert_eq!(
            expected.bits(),
            actual.bits(),
            "{name}: phase {index} expected {:08x}, got {:08x}",
            expected.bits(),
            actual.bits()
        );
    }
}

fn assert_other_voices_unchanged(osc: &Oscillator, case: &RenderCase) {
    let other_channel = usize::from(case.channel == 0);
    let other_voice = usize::from(case.voice == 0);

    assert_eq!(
        osc.voices.at(other_channel, case.voice).phases[0].bits(),
        DECOY_PHASE,
        "{}: rendered the other channel",
        case.name
    );
    assert_eq!(
        osc.voices.at(case.channel, other_voice).phases[0].bits(),
        DECOY_PHASE,
        "{}: rendered another voice",
        case.name
    );
}

/// Rendered samples and advanced phases match an independent scalar
/// Catmull-Rom lookup. Covers both table sizes, a size crossfade, partial SIMD
/// lanes, a mid-block range, and reading the spectrum channel's wavetable.
#[test]
fn render_voice_samples_matches_scalar_reference() {
    let mut mid_block = render_case("mid-block range", 7, WaveformSize::Half, WaveformSize::Full);
    mid_block.start = 6;
    mid_block.end = 28;

    let mut other_channel = render_case("right channel", 3, WaveformSize::Half, WaveformSize::Half);
    other_channel.channel = 1;

    let mut other_wave = render_case("sixteen voices", 16, WaveformSize::Full, WaveformSize::Full);
    other_wave.samples = 16;
    other_wave.end = 16;
    other_wave.voice = 2;
    other_wave.wave_channel = 1;

    let mut sized = render_case(
        "partial chunk, full to half",
        5,
        WaveformSize::Full,
        WaveformSize::Half,
    );
    sized.samples = 24;
    sized.end = 24;

    let cases = [
        render_case(
            "one voice, full table",
            1,
            WaveformSize::Full,
            WaveformSize::Full,
        ),
        render_case(
            "simd chunk, half table",
            4,
            WaveformSize::Half,
            WaveformSize::Half,
        ),
        sized,
        mid_block,
        other_wave,
        other_channel,
        render_case(
            "thirteen voices, half to full",
            13,
            WaveformSize::Half,
            WaveformSize::Full,
        ),
    ];

    let mut osc = Oscillator::new(1);

    for case in cases {
        let wave_from = patterned_wave(case.from_size, 1.7);
        let wave_to = patterned_wave(case.to_size, 2.9);
        let (expected, expected_phases) = reference_render(&case, &wave_from, &wave_to);

        load_case(&mut osc, &case, &wave_from, &wave_to);

        let mut actual = vec![RENDER_SENTINEL; case.samples];
        let ctx = render_ctx(&case);
        osc.render_voice_samples(&ctx, &mut actual, case.start, case.end);

        assert_samples_close(case.name, &expected, &actual);
        assert_phases(
            case.name,
            &expected_phases,
            &osc.voices.at(case.channel, case.voice).phases,
        );
        assert_other_voices_unchanged(&osc, &case);
    }
}

/// A phase-steal split must keep going from the phases of the first slice.
/// `buff_t` is still the index inside the whole block.
#[test]
fn render_voice_samples_continues_across_a_split_range() {
    let mut case = render_case("split block", 6, WaveformSize::Half, WaveformSize::Full);
    case.samples = 48;
    case.end = 48;

    let wave_from = patterned_wave(case.from_size, 1.7);
    let wave_to = patterned_wave(case.to_size, 2.9);
    let (expected, expected_phases) = reference_render(&case, &wave_from, &wave_to);

    let mut osc = Oscillator::new(1);
    load_case(&mut osc, &case, &wave_from, &wave_to);

    let mut actual = vec![RENDER_SENTINEL; case.samples];
    let ctx = render_ctx(&case);
    osc.render_voice_samples(&ctx, &mut actual, 0, 17);
    osc.render_voice_samples(&ctx, &mut actual, 17, 48);

    assert_samples_close(case.name, &expected, &actual);
    assert_phases(
        case.name,
        &expected_phases,
        &osc.voices.at(case.channel, case.voice).phases,
    );
}

#[test]
fn render_voice_samples_empty_range_is_a_no_op() {
    let case = render_case("empty", 4, WaveformSize::Full, WaveformSize::Half);
    let wave_from = patterned_wave(case.from_size, 1.7);
    let wave_to = patterned_wave(case.to_size, 2.9);
    let mut osc = Oscillator::new(1);
    load_case(&mut osc, &case, &wave_from, &wave_to);

    let phases_before = osc.voices.at(case.channel, case.voice).phases;
    let mut output = vec![RENDER_SENTINEL; case.samples];
    let ctx = render_ctx(&case);

    osc.render_voice_samples(&ctx, &mut output, 4, 4);
    osc.render_voice_samples(&ctx, &mut output, 10, 4);

    assert!(output.iter().all(|sample| *sample == RENDER_SENTINEL));
    assert_eq!(
        osc.voices.at(case.channel, case.voice).phases,
        phases_before
    );
}
