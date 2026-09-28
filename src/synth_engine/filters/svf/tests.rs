use std::f32::consts::{PI, TAU};

use crate::utils::freq_to_c4_pitch;

use super::{SvfFilter, g_table, *};

fn approx_g(freq_ratio: Sample) -> Sample {
    g_table().at((freq_ratio / RANGE_SCALE).min(1.0))
}

const MAX_FREQ_RATIO: Sample = 0.499;
const SAMPLE_RATE: Sample = 48_000.0;
const CUTOFF: Sample = 1_000.0;
/// Butterworth Q. A 2-pole low-pass is −6 dB at the cutoff.
const FLAT_Q: Sample = 0.5;

#[test]
fn prewarped_g_tracks_tan() {
    let mut prev = 0.0;
    let mut worst_interior = (0.0, 0.0);
    let step = (G_TABLE_INTERVALS as Sample).recip();

    for i in 0..=20_000 {
        let ratio = MAX_FREQ_RATIO * i as Sample / 20_000.0;
        let expected = (ratio * PI).tan();
        let actual = approx_g(ratio);
        let omega_expected = expected.atan();
        let rel = if omega_expected == 0.0 {
            (actual.atan() - omega_expected).abs()
        } else {
            (actual.atan() - omega_expected).abs() / omega_expected
        };

        assert!(actual.is_finite(), "non-finite g at ratio {ratio}");
        assert!(
            actual + 1e-4 >= prev,
            "g decreased at ratio {ratio}: {actual} < {prev}"
        );

        if rel > worst_interior.1 {
            worst_interior = (ratio, rel);
        }

        prev = actual;
    }

    assert!(
        worst_interior.1 < 1e-4,
        "worst interior relative cutoff error {} at freq/sample_rate {}",
        worst_interior.1,
        worst_interior.0
    );

    assert_eq!(approx_g(0.0), 0.0);
    let knot = 256.0 * step;
    assert_eq!(
        g_table().at(knot),
        ((RANGE_SCALE * knot).min(0.499) * PI).tan()
    );
    assert_eq!(approx_g(0.75), approx_g(1.0));
}

#[test]
fn process_clamps_frequency_ratio_to_the_tan_limit() {
    let q = [FLAT_Q];
    let gain = [1.0];
    let input = [1.0];

    let run = |ratio: Sample| {
        let cutoff = [freq_to_c4_pitch(ratio * SAMPLE_RATE)];
        let mut state = SvfState::new(SvfType::LowPass12);
        let mut output = [0.0];

        state.process(SAMPLE_RATE, &input, &cutoff, &q, &gain, &mut output);

        output[0]
    };

    assert_eq!(run(0.6), run(1.0));
    assert_ne!(run(0.01), run(1.0));
}

#[test]
fn prewarped_g_is_accurate_at_low_cutoffs() {
    for sample_rate in [44_100.0, 48_000.0, 96_000.0, 192_000.0] {
        for freq in [5.0, 10.0, 20.0, 40.0, 80.0] {
            let ratio: Sample = freq / sample_rate;
            let expected = (ratio * PI).tan();
            let rel = (approx_g(ratio) - expected).abs() / expected;

            assert!(
                rel < 1e-4,
                "{freq} Hz at {sample_rate} Hz: relative error {rel}"
            );
        }
    }
}

fn g_and_k(cutoff: Sample, q: Sample) -> (Sample, Sample) {
    let g = (PI * (cutoff * SAMPLE_RATE.recip()).min(0.499)).tan();

    (g, q.recip())
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
        let y = state.tick(g, k, gain, x);

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
    for filter_type in [SvfType::BandPass6, SvfType::BandPass12] {
        for q in [0.5, 4.0, 16.0] {
            assert!(
                (analog_gain(filter_type, q, CUTOFF) - 1.0).abs() < 1e-5,
                "{filter_type:?} q={q}"
            );
        }
    }
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
        let y = state.tick(g, k, gain, x);

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

        if filter_type == SvfType::Peaking {
            assert_impulse_decays(filter_type, 4.0);
            assert_impulse_decays(filter_type, 0.25);
        }
    }
}

#[test]
fn reset_clears_memory() {
    let (g, k) = g_and_k(CUTOFF, 2.0);
    let mut state = SvfState::new(SvfType::LowPass24);

    for _ in 0..100 {
        state.tick(g, k, 1.0, 1.0);
    }

    state.reset();

    assert_eq!(state.tick(g, k, 1.0, 0.0), 0.0);
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
    let q = [2.0];
    let gain = [gain];
    let mut state = SvfState::new(filter_type);
    let mut output = [0.0];

    for _ in 0..8_000 {
        state.process(SAMPLE_RATE, &[1.0], &cutoff, &q, &gain, &mut output);
    }

    output[0]
}

#[test]
fn peaking_dc_stays_unity_while_drive_scales_the_other_types() {
    assert!((settled_dc(SvfType::Peaking, 4.0) - 1.0).abs() < 1e-3);
    assert!((settled_dc(SvfType::Notch, 4.0) - 4.0).abs() < 1e-2);
    assert!((settled_dc(SvfType::LowPass12, 4.0) - 4.0).abs() < 1e-2);
}

#[test]
fn types_round_trip_through_serde() {
    for filter_type in SvfType::ALL {
        let json = serde_json::to_string(&filter_type).unwrap();
        let back: SvfType = serde_json::from_str(&json).unwrap();

        assert_eq!(back, filter_type);
    }
}
