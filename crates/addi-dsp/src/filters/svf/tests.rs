use std::f32::consts::{PI, TAU};

use crate::{ComplexSample, filters::control::MAX_PRE_Q, units::freq_to_c4_pitch};

use super::{
    SvfFilter,
    sections::{
        BandPass, HighPass, HighShelf, Integrator, LowPass, LowShelf, Notch, OnePoleHighPass,
        OnePoleIntegrator, OnePoleLowPass, Peaking, ShelfCoeffs,
    },
    *,
};

const SAMPLE_RATE: Sample = 48_000.0;
const CUTOFF: Sample = 1_000.0;
/// Butterworth Q. A 2-pole low-pass is −6 dB at the cutoff.
const FLAT_Q: Sample = 0.5;

#[test]
fn process_clamps_frequency_ratio_to_the_tan_limit() {
    let k = [FLAT_Q.recip()];
    let gain = [1.0];
    let input = [1.0];

    let run = |ratio: Sample| {
        let cutoff = [freq_to_c4_pitch(ratio * SAMPLE_RATE)];
        let mut state = SvfState::new(SvfType::LowPass12);
        let mut output = [0.0];

        let pre_k = [MAX_PRE_Q.recip()];

        state.process(SAMPLE_RATE, &input, &cutoff, &k, &pre_k, &gain, &mut output);

        output[0]
    };

    assert_eq!(run(0.6), run(1.0));
    assert_ne!(run(0.01), run(1.0));
}

fn g_and_k(cutoff: Sample, q: Sample) -> (Sample, Sample) {
    let g = (PI * (cutoff * SAMPLE_RATE.recip()).min(0.499)).tan();

    (g, q.recip())
}

fn tick_params(g: Sample, k: Sample, gain: Sample, input: Sample) -> TickParams {
    TickParams {
        g,
        k,
        pre_k: MAX_PRE_Q.recip(),
        gain,
        input,
    }
}

/// Steady-state amplitude of the filter output for a unit sine at `freq`.
fn measured_gain(filter_type: SvfType, q: Sample, gain: Sample, freq: Sample) -> Sample {
    let (g, k) = g_and_k(CUTOFF, q);
    let mut state = SvfState::new(filter_type);
    let settle = SAMPLE_RATE as usize;
    let measure = SAMPLE_RATE as usize / 4;
    let mut sum_sq = 0.0f64;

    for n in 0..settle + measure {
        let x = (TAU * freq * n as Sample / SAMPLE_RATE).sin();
        let y = state.tick(&tick_params(g, k, gain, x));

        if n >= settle {
            sum_sq += (y as f64) * (y as f64);
        }
    }

    ((sum_sq / measure as f64).sqrt() * std::f64::consts::SQRT_2) as Sample
}

fn predicted_gain(filter_type: SvfType, q: Sample, gain: Sample, freq: Sample) -> Sample {
    let w = (PI * freq / SAMPLE_RATE).tan();
    let g = (PI * CUTOFF / SAMPLE_RATE).tan();

    let response = SvfResponse {
        filter_type,
        q,
        pre_q: MAX_PRE_Q,
        cutoff: g,
        gain,
    };

    response.at(w).norm()
}

fn analog_gain(filter_type: SvfType, q: Sample, freq: Sample) -> Sample {
    analog_gain_with(filter_type, q, 1.0, freq)
}

fn analog_gain_with(filter_type: SvfType, q: Sample, gain: Sample, freq: Sample) -> Sample {
    let response = SvfResponse {
        filter_type,
        q,
        pre_q: MAX_PRE_Q,
        cutoff: CUTOFF,
        gain,
    };

    response.at(freq).norm()
}

fn assert_close(measured: Sample, predicted: Sample, ctx: &str) {
    let tolerance = (predicted * 0.03).max(2e-3);

    assert!(
        (measured - predicted).abs() <= tolerance,
        "{ctx}: measured {measured} vs predicted {predicted}"
    );
}

#[test]
fn tick_matches_bilinear_response_for_all_types() {
    for filter_type in SvfType::ALL {
        for q in [FLAT_Q, 1.0, 4.0] {
            for freq in [250.0, 1_000.0, 4_000.0] {
                assert_close(
                    measured_gain(filter_type, q, 1.0, freq),
                    predicted_gain(filter_type, q, 1.0, freq),
                    &format!("{filter_type:?} q={q} f={freq}"),
                );
            }
        }
    }
}

#[test]
fn analog_prototype_dc_and_cutoff_gains() {
    for filter_type in [SvfType::LowPass12, SvfType::LowPass18, SvfType::LowPass24] {
        assert!(
            (analog_gain(filter_type, FLAT_Q, 0.0) - 1.0).abs() < 1e-6,
            "{filter_type:?}"
        );
    }

    for filter_type in [
        SvfType::HighPass12,
        SvfType::HighPass18,
        SvfType::HighPass24,
    ] {
        assert!(
            analog_gain(filter_type, FLAT_Q, 0.0) < 1e-6,
            "{filter_type:?}"
        );
    }

    // Normalized band-pass stages are unity at the cutoff for any Q.
    for filter_type in [
        SvfType::BandPass6,
        SvfType::BandPass12,
        SvfType::BandPass18,
        SvfType::BandPass24,
    ] {
        for q in [0.5, 4.0, 16.0] {
            assert!(
                (analog_gain(filter_type, q, CUTOFF) - 1.0).abs() < 1e-5,
                "{filter_type:?} q={q}"
            );
        }
    }
}

#[test]
fn lowpass24_cutoff_gain_is_the_product_of_the_stage_qs() {
    let q = 2.0;
    let pre_q = 0.5;
    let response = SvfResponse {
        filter_type: SvfType::LowPass24,
        q,
        pre_q,
        cutoff: CUTOFF,
        gain: 1.0,
    };

    assert!((response.at(CUTOFF).norm() - q * pre_q).abs() < 1e-5);
}

#[test]
fn zero_resonance_lowpass_is_minus_6db_at_cutoff() {
    // |H(fc)| of a 2-pole low-pass equals its Q, so Q = 0.5 is −6 dB.
    let gain = analog_gain(SvfType::LowPass12, FLAT_Q, CUTOFF);

    assert!((gain - FLAT_Q).abs() < 1e-5, "{gain}");
}

#[test]
fn steeper_types_roll_off_faster() {
    let far = 8.0 * CUTOFF;
    let gain = |filter_type| analog_gain(filter_type, FLAT_Q, far);

    assert!(gain(SvfType::LowPass12) > gain(SvfType::LowPass18));
    assert!(gain(SvfType::LowPass18) > gain(SvfType::LowPass24));
    assert!(gain(SvfType::BandPass6) > gain(SvfType::BandPass12));
    assert!(gain(SvfType::BandPass12) > gain(SvfType::BandPass18));
    assert!(gain(SvfType::BandPass18) > gain(SvfType::BandPass24));

    let near = CUTOFF / 8.0;
    let gain = |filter_type| analog_gain(filter_type, FLAT_Q, near);

    assert!(gain(SvfType::HighPass12) > gain(SvfType::HighPass18));
    assert!(gain(SvfType::HighPass18) > gain(SvfType::HighPass24));
}

fn assert_impulse_decays(filter_type: SvfType, gain: Sample) {
    let (g, k) = g_and_k(CUTOFF, 16.0);
    let mut state = SvfState::new(filter_type);
    let mut peak_early = 0.0f32;
    let mut peak_late = 0.0f32;

    for n in 0..SAMPLE_RATE as usize {
        let x = if n == 0 { 1.0 } else { 0.0 };
        let y = state.tick(&tick_params(g, k, gain, x));

        assert!(y.is_finite(), "{filter_type:?} gain={gain} at {n}");

        if n < 2_000 {
            peak_early = peak_early.max(y.abs());
        } else if n >= 40_000 {
            peak_late = peak_late.max(y.abs());
        }
    }

    assert!(
        peak_late < peak_early * 0.01,
        "{filter_type:?} gain={gain}: early {peak_early} late {peak_late}"
    );
}

#[test]
fn high_resonance_impulse_decays_and_stays_finite() {
    for filter_type in SvfType::ALL {
        assert_impulse_decays(filter_type, 1.0);

        if matches!(
            filter_type,
            SvfType::Peaking
                | SvfType::LowShelf12
                | SvfType::LowShelf24
                | SvfType::HighShelf12
                | SvfType::HighShelf24
        ) {
            assert_impulse_decays(filter_type, 4.0);
            assert_impulse_decays(filter_type, 0.25);
        }
    }
}

#[test]
fn fresh_state_has_no_memory() {
    let (g, k) = g_and_k(CUTOFF, 2.0);
    let mut state = SvfState::new(SvfType::LowPass24);

    for _ in 0..100 {
        state.tick(&tick_params(g, k, 1.0, 1.0));
    }

    state = SvfState::new(SvfType::LowPass24);

    assert_eq!(state.tick(&tick_params(g, k, 1.0, 0.0)), 0.0);
}

#[test]
fn notch_passes_away_from_cutoff_and_nulls_it() {
    assert!((analog_gain(SvfType::Notch, FLAT_Q, 0.0) - 1.0).abs() < 1e-6);
    assert!(analog_gain(SvfType::Notch, 4.0, CUTOFF) < 1e-6);
    assert!((analog_gain(SvfType::Notch, FLAT_Q, 100.0 * CUTOFF) - 1.0).abs() < 1e-3);
}

#[test]
fn peaking_is_unity_away_from_cutoff_and_gain_at_cutoff() {
    for gain in [0.25, 4.0] {
        assert!(
            (analog_gain_with(SvfType::Peaking, 2.0, gain, 0.0) - 1.0).abs() < 1e-6,
            "dc gain={gain}"
        );
        assert!(
            (analog_gain_with(SvfType::Peaking, 2.0, gain, CUTOFF) - gain).abs() < 1e-5,
            "cutoff gain={gain}"
        );
        assert!(
            (analog_gain_with(SvfType::Peaking, 2.0, gain, 100.0 * CUTOFF) - 1.0).abs() < 1e-3,
            "high gain={gain}"
        );
    }
}

#[test]
fn peaking_tick_matches_bilinear_response() {
    for gain in [0.25, 4.0] {
        for q in [FLAT_Q, 1.0, 4.0] {
            for freq in [250.0, 1_000.0, 4_000.0] {
                assert_close(
                    measured_gain(SvfType::Peaking, q, gain, freq),
                    predicted_gain(SvfType::Peaking, q, gain, freq),
                    &format!("peaking gain={gain} q={q} f={freq}"),
                );
            }
        }
    }
}

fn settled_dc(filter_type: SvfType, gain: Sample) -> Sample {
    let cutoff = [freq_to_c4_pitch(CUTOFF)];
    let k = [0.5];
    let gain = [gain];
    let mut state = SvfState::new(filter_type);
    let mut output = [0.0];

    for _ in 0..8_000 {
        let pre_k = [MAX_PRE_Q.recip()];

        state.process(SAMPLE_RATE, &[1.0], &cutoff, &k, &pre_k, &gain, &mut output);
    }

    output[0]
}

#[test]
fn peaking_dc_stays_unity_while_drive_scales_the_other_types() {
    assert!((settled_dc(SvfType::Peaking, 4.0) - 1.0).abs() < 1e-3);
    assert!((settled_dc(SvfType::Notch, 4.0) - 4.0).abs() < 1e-2);
    assert!((settled_dc(SvfType::LowPass12, 4.0) - 4.0).abs() < 1e-2);
}

fn analog_h(filter_type: SvfType, q: Sample, gain: Sample, freq: Sample) -> ComplexSample {
    SvfResponse {
        filter_type,
        q,
        pre_q: MAX_PRE_Q,
        cutoff: CUTOFF,
        gain,
    }
    .at(freq)
}

#[test]
fn lowshelf_boosts_dc_and_is_unity_at_high_freq() {
    for filter_type in [SvfType::LowShelf12, SvfType::LowShelf24] {
        for gain in [0.25, 4.0] {
            assert!(
                (analog_gain_with(filter_type, 2.0, gain, 0.0) - gain).abs() < 1e-5,
                "{filter_type:?} dc gain={gain}"
            );
            assert!(
                (analog_gain_with(filter_type, 2.0, gain, CUTOFF) - gain.sqrt()).abs() < 1e-5,
                "{filter_type:?} cutoff gain={gain}"
            );
            assert!(
                (analog_gain_with(filter_type, 2.0, gain, 100.0 * CUTOFF) - 1.0).abs() < 1e-3,
                "{filter_type:?} high gain={gain}"
            );
        }
    }
}

#[test]
fn highshelf_is_unity_at_dc_and_boosts_high_freq() {
    for filter_type in [SvfType::HighShelf12, SvfType::HighShelf24] {
        for gain in [0.25, 4.0] {
            assert!(
                (analog_gain_with(filter_type, 2.0, gain, 0.0) - 1.0).abs() < 1e-5,
                "{filter_type:?} dc gain={gain}"
            );
            assert!(
                (analog_gain_with(filter_type, 2.0, gain, CUTOFF) - gain.sqrt()).abs() < 1e-5,
                "{filter_type:?} cutoff gain={gain}"
            );
            assert!(
                (analog_gain_with(filter_type, 2.0, gain, 100.0 * CUTOFF) - gain).abs() < 1e-3,
                "{filter_type:?} high gain={gain}"
            );
        }
    }
}

#[test]
fn shelves_are_identity_at_unity_gain() {
    for filter_type in [
        SvfType::LowShelf12,
        SvfType::LowShelf24,
        SvfType::HighShelf12,
        SvfType::HighShelf24,
    ] {
        for freq in [0.0, 0.25 * CUTOFF, CUTOFF, 4.0 * CUTOFF] {
            let h = analog_h(filter_type, 2.0, 1.0, freq);

            assert!((h.re - 1.0).abs() < 1e-6, "{filter_type:?} f={freq}");
            assert!(h.im.abs() < 1e-6, "{filter_type:?} f={freq}");
        }
    }
}

#[test]
fn shelf_boost_and_cut_are_inverses() {
    let freq = 1.7 * CUTOFF;
    let gain: Sample = 4.0;
    let cut = gain.recip();

    for filter_type in [
        SvfType::LowShelf12,
        SvfType::LowShelf24,
        SvfType::HighShelf12,
        SvfType::HighShelf24,
    ] {
        let product =
            analog_h(filter_type, 2.0, gain, freq) * analog_h(filter_type, 2.0, cut, freq);

        assert!(
            (product.re - 1.0).abs() < 1e-5 && product.im.abs() < 1e-5,
            "{filter_type:?} {product}"
        );
    }
}

#[test]
fn low_and_high_shelf_product_is_flat_gain() {
    let freq = 1.7 * CUTOFF;
    let gain = 4.0;

    for (low, high) in [
        (SvfType::LowShelf12, SvfType::HighShelf12),
        (SvfType::LowShelf24, SvfType::HighShelf24),
    ] {
        let product = analog_h(low, 2.0, gain, freq) * analog_h(high, 2.0, gain, freq);

        assert!((product.re - gain).abs() < 1e-4, "{low:?} {product}");
        assert!(product.im.abs() < 1e-4, "{low:?} {product}");
    }
}

#[test]
fn shelf_tick_matches_bilinear_response() {
    for filter_type in [
        SvfType::LowShelf12,
        SvfType::LowShelf24,
        SvfType::HighShelf12,
        SvfType::HighShelf24,
    ] {
        for gain in [0.25, 4.0] {
            for q in [FLAT_Q, 1.0, 4.0] {
                for freq in [250.0, 1_000.0, 4_000.0] {
                    assert_close(
                        measured_gain(filter_type, q, gain, freq),
                        predicted_gain(filter_type, q, gain, freq),
                        &format!("{filter_type:?} gain={gain} q={q} f={freq}"),
                    );
                }
            }
        }
    }
}

#[test]
fn shelf_dc_follows_the_shelf_gain() {
    for filter_type in [SvfType::LowShelf12, SvfType::LowShelf24] {
        assert!(
            (settled_dc(filter_type, 4.0) - 4.0).abs() < 1e-2,
            "{filter_type:?}"
        );
        assert!(
            (settled_dc(filter_type, 0.25) - 0.25).abs() < 1e-3,
            "{filter_type:?}"
        );
    }

    for filter_type in [SvfType::HighShelf12, SvfType::HighShelf24] {
        assert!(
            (settled_dc(filter_type, 4.0) - 1.0).abs() < 1e-3,
            "{filter_type:?}"
        );
        assert!(
            (settled_dc(filter_type, 0.25) - 1.0).abs() < 1e-3,
            "{filter_type:?}"
        );
    }
}

#[test]
fn steeper_shelves_transition_faster() {
    let gain = 10.0;
    let below = 0.5 * CUTOFF;
    let above = 2.0 * CUTOFF;

    assert!(
        analog_gain_with(SvfType::LowShelf24, FLAT_Q, gain, above)
            < analog_gain_with(SvfType::LowShelf12, FLAT_Q, gain, above)
    );
    assert!(
        analog_gain_with(SvfType::LowShelf24, FLAT_Q, gain, below)
            > analog_gain_with(SvfType::LowShelf12, FLAT_Q, gain, below)
    );
    assert!(
        analog_gain_with(SvfType::HighShelf24, FLAT_Q, gain, below)
            < analog_gain_with(SvfType::HighShelf12, FLAT_Q, gain, below)
    );
    assert!(
        analog_gain_with(SvfType::HighShelf24, FLAT_Q, gain, above)
            > analog_gain_with(SvfType::HighShelf12, FLAT_Q, gain, above)
    );
}

#[test]
fn types_round_trip_through_serde() {
    for filter_type in SvfType::ALL {
        let json = serde_json::to_string(&filter_type).unwrap();
        let back: SvfType = serde_json::from_str(&json).unwrap();

        assert_eq!(back, filter_type);
    }
}

#[test]
fn default_state_is_lowpass12() {
    assert_eq!(SvfType::default(), SvfType::LowPass12);

    let (g, k) = g_and_k(CUTOFF, 2.0);
    let mut state = SvfState::default();
    let mut lowpass = SvfState::new(SvfType::LowPass12);

    for input in [0.0, 1.0, -1.0, 0.3] {
        assert_eq!(
            state.tick(&tick_params(g, k, 1.0, input)),
            lowpass.tick(&tick_params(g, k, 1.0, input))
        );
    }
}

fn drive(state: &mut SvfState) {
    let (g, k) = g_and_k(CUTOFF, 4.0);

    for _ in 0..32 {
        state.tick(&tick_params(g, k, 2.0, 1.0));
    }
}

fn tick_sequence(state: &mut SvfState) -> [Sample; 4] {
    let (g, k) = g_and_k(CUTOFF, 4.0);
    let mut output = [0.0; 4];

    for (out, input) in output.iter_mut().zip([0.0, 1.0, -0.5, 0.25]) {
        *out = state.tick(&tick_params(g, k, 2.0, input));
    }

    output
}

#[test]
fn set_type_keeps_integrator_state_when_the_type_is_unchanged() {
    for filter_type in SvfType::ALL {
        let mut driven = SvfState::new(filter_type);

        drive(&mut driven);

        let mut kept = driven;
        let mut untouched = driven;
        let mut fresh = SvfState::new(filter_type);

        kept.set_type(filter_type);

        let kept_out = tick_sequence(&mut kept);
        let untouched_out = tick_sequence(&mut untouched);
        let fresh_out = tick_sequence(&mut fresh);

        assert_eq!(kept_out, untouched_out, "{filter_type:?}");
        assert!(
            kept_out
                .iter()
                .zip(fresh_out)
                .any(|(kept, fresh)| (kept - fresh).abs() > 1e-4),
            "{filter_type:?} lost its state"
        );
    }
}

#[test]
fn set_type_clears_integrator_state_when_the_type_changes() {
    let types = SvfType::ALL;

    for (i, &filter_type) in types.iter().enumerate() {
        let next = types[(i + 1) % types.len()];
        let mut state = SvfState::new(filter_type);

        drive(&mut state);
        state.set_type(next);

        let mut fresh = SvfState::new(next);

        assert_eq!(
            tick_sequence(&mut state),
            tick_sequence(&mut fresh),
            "{filter_type:?} -> {next:?}"
        );
    }
}

/// `(g, k, input)`. Coefficients change every step so the state update is part of the check.
const SECTION_STEPS: [(f64, f64, f64); 6] = [
    (0.0, 0.5, 1.0),
    (0.15, 0.5, 0.5),
    (0.4, 1.0, -1.0),
    (0.9, 0.2, 0.25),
    (0.25, 2.0, 0.0),
    (1.2, 0.05, -0.75),
];

fn assert_sample_near(actual: Sample, expected: f64, ctx: &str) {
    let error = (f64::from(actual) - expected).abs();
    let tolerance = expected.abs() * 1e-5 + 1e-6;

    assert!(
        error <= tolerance,
        "{ctx}: actual {actual} expected {expected}"
    );
}

/// Cytomic trapezoidal pair, in `f64` and in the `a1`/`a2`/`a3` form.
#[derive(Default)]
struct RefIntegrator {
    ic1eq: f64,
    ic2eq: f64,
}

impl RefIntegrator {
    fn tick(&mut self, g: f64, k: f64, v0: f64) -> (f64, f64) {
        let a1 = 1.0 / (1.0 + g * (g + k));
        let a2 = g * a1;
        let a3 = g * a2;
        let v3 = v0 - self.ic2eq;
        let v1 = a1 * self.ic1eq + a2 * v3;
        let v2 = self.ic2eq + a2 * self.ic1eq + a3 * v3;

        self.ic1eq = 2.0 * v1 - self.ic1eq;
        self.ic2eq = 2.0 * v2 - self.ic2eq;

        (v1, v2)
    }
}

#[derive(Default)]
struct RefOnePole {
    s: f64,
}

impl RefOnePole {
    fn tick(&mut self, g: f64, input: f64) -> f64 {
        let a = g / (1.0 + g);
        let v = (input - self.s) * a;
        let lp = self.s + v;

        self.s += 2.0 * v;

        lp
    }
}

struct RefShelf {
    a: f64,
    sqrt_a: f64,
    gain: f64,
}

impl RefShelf {
    fn new(gain: f64) -> Self {
        let gain = gain.max(1e-4);
        let a = gain.sqrt();

        Self {
            a,
            sqrt_a: a.sqrt(),
            gain,
        }
    }

    fn cascaded(gain: f64) -> Self {
        Self::new(gain.max(1e-4).sqrt())
    }
}

#[test]
fn trapezoidal_integrator_matches_reference() {
    let mut integrator = Integrator::default();
    let mut reference = RefIntegrator::default();

    for (step, &(g, k, input)) in SECTION_STEPS.iter().enumerate() {
        let (v1, v2) = integrator.tick(g as Sample, k as Sample, input as Sample);
        let (expected_v1, expected_v2) = reference.tick(g, k, input);

        assert_sample_near(v1, expected_v1, &format!("integrator v1 step {step}"));
        assert_sample_near(v2, expected_v2, &format!("integrator v2 step {step}"));
    }
}

#[test]
fn one_pole_integrator_matches_reference() {
    let mut integrator = OnePoleIntegrator::default();
    let mut reference = RefOnePole::default();

    for (step, &(g, _, input)) in SECTION_STEPS.iter().enumerate() {
        let lp = integrator.tick(g as Sample, input as Sample);

        assert_sample_near(
            lp,
            reference.tick(g, input),
            &format!("one-pole step {step}"),
        );
    }
}

#[test]
fn two_pole_sections_match_reference() {
    let mut low_pass = LowPass::default();
    let mut high_pass = HighPass::default();
    let mut band_pass = BandPass::default();
    let mut notch = Notch::default();
    let mut low_ref = RefIntegrator::default();
    let mut high_ref = RefIntegrator::default();
    let mut band_ref = RefIntegrator::default();
    let mut notch_ref = RefIntegrator::default();

    for (step, &(g, k, input)) in SECTION_STEPS.iter().enumerate() {
        let g_s = g as Sample;
        let k_s = k as Sample;
        let input_s = input as Sample;

        let (_v1, low) = low_ref.tick(g, k, input);
        let (v1, v2) = high_ref.tick(g, k, input);
        let (band_v1, _v2) = band_ref.tick(g, k, input);
        let (notch_v1, _v2) = notch_ref.tick(g, k, input);

        assert_sample_near(
            low_pass.tick(g_s, k_s, input_s),
            low,
            &format!("lowpass step {step}"),
        );
        assert_sample_near(
            high_pass.tick(g_s, k_s, input_s),
            input - k * v1 - v2,
            &format!("highpass step {step}"),
        );
        assert_sample_near(
            band_pass.tick(g_s, k_s, input_s),
            k * band_v1,
            &format!("bandpass step {step}"),
        );
        assert_sample_near(
            notch.tick(g_s, k_s, input_s),
            input - k * notch_v1,
            &format!("notch step {step}"),
        );
    }
}

#[test]
fn peaking_section_matches_reference() {
    for gain in [1.0_f64, 4.0, 0.25, 0.0, -1.0] {
        let mut peaking = Peaking::default();
        let mut reference = RefIntegrator::default();
        let clamped = gain.max(1e-4);
        let k_scale = clamped.sqrt();

        for (step, &(g, k, input)) in SECTION_STEPS.iter().enumerate() {
            let (v1, _v2) = reference.tick(g, k / k_scale, input);
            let expected = input + (clamped - 1.0) * (k / k_scale) * v1;

            assert_sample_near(
                peaking.tick(g as Sample, k as Sample, gain as Sample, input as Sample),
                expected,
                &format!("peaking gain={gain} step {step}"),
            );
        }
    }
}

#[test]
fn shelf_sections_match_reference() {
    for (label, coeffs_for) in [
        ("section", RefShelf::new as fn(f64) -> RefShelf),
        ("cascaded", RefShelf::cascaded as fn(f64) -> RefShelf),
    ] {
        for gain in [1.0_f64, 4.0, 0.25, 0.0, -1.0] {
            let coeffs = coeffs_for(gain);
            let shelf = if label == "section" {
                ShelfCoeffs::new(gain as Sample)
            } else {
                ShelfCoeffs::cascaded(gain as Sample)
            };
            let mut low_shelf = LowShelf::default();
            let mut high_shelf = HighShelf::default();
            let mut low_ref = RefIntegrator::default();
            let mut high_ref = RefIntegrator::default();

            for (step, &(g, k, input)) in SECTION_STEPS.iter().enumerate() {
                let (low_v1, low_v2) = low_ref.tick(g / coeffs.sqrt_a, k, input);
                let (high_v1, high_v2) = high_ref.tick(g * coeffs.sqrt_a, k, input);
                let low = input + (coeffs.a - 1.0) * k * low_v1 + (coeffs.gain - 1.0) * low_v2;
                let high = coeffs.gain * input
                    + (coeffs.a - coeffs.gain) * k * high_v1
                    + (1.0 - coeffs.gain) * high_v2;

                assert_sample_near(
                    low_shelf.tick(g as Sample, k as Sample, shelf, input as Sample),
                    low,
                    &format!("lowshelf {label} gain={gain} step {step}"),
                );
                assert_sample_near(
                    high_shelf.tick(g as Sample, k as Sample, shelf, input as Sample),
                    high,
                    &format!("highshelf {label} gain={gain} step {step}"),
                );
            }
        }
    }
}

#[test]
fn one_pole_sections_match_reference() {
    let mut low_pass = OnePoleLowPass::default();
    let mut high_pass = OnePoleHighPass::default();
    let mut low_ref = RefOnePole::default();
    let mut high_ref = RefOnePole::default();

    for (step, &(g, _, input)) in SECTION_STEPS.iter().enumerate() {
        let low = low_ref.tick(g, input);
        let high = input - high_ref.tick(g, input);

        assert_sample_near(
            low_pass.tick(g as Sample, input as Sample),
            low,
            &format!("one-pole lowpass step {step}"),
        );
        assert_sample_near(
            high_pass.tick(g as Sample, input as Sample),
            high,
            &format!("one-pole highpass step {step}"),
        );
    }
}
