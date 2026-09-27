use std::f32::consts::{PI, TAU};

use super::{SvfFilter, *};

const SAMPLE_RATE: Sample = 48_000.0;
const CUTOFF: Sample = 1_000.0;

fn g_and_k(cutoff: Sample, q: Sample) -> (Sample, Sample) {
    let g = (PI * (cutoff * SAMPLE_RATE.recip()).min(0.499)).tan();

    (g, q.recip())
}

/// Steady-state amplitude of the filter output for a unit sine at `freq`.
fn measured_gain(filter_type: SvfType, q: Sample, freq: Sample) -> Sample {
    let (g, k) = g_and_k(CUTOFF, q);
    let mut state = SvfState::new(filter_type);
    let settle = SAMPLE_RATE as usize;
    let measure = SAMPLE_RATE as usize / 4;
    let mut sum_sq = 0.0f64;

    for n in 0..settle + measure {
        let x = (TAU * freq * n as Sample / SAMPLE_RATE).sin();
        let y = state.tick(g, k, x);

        if n >= settle {
            sum_sq += (y as f64) * (y as f64);
        }
    }

    ((sum_sq / measure as f64).sqrt() * std::f64::consts::SQRT_2) as Sample
}

fn resonance_for_q(q: Sample) -> Sample {
    ((q - ZERO_RESONANCE_Q) / (MAX_RESONANCE_Q - ZERO_RESONANCE_Q)).cbrt()
}

fn predicted_gain(filter_type: SvfType, q: Sample, freq: Sample) -> Sample {
    SvfResponse {
        filter_type,
        resonance: resonance_for_q(q),
    }
    .at(SvfResponse::digital_s(freq, CUTOFF, SAMPLE_RATE))
    .norm()
}

fn analog_gain(filter_type: SvfType, q: Sample, freq: Sample) -> Sample {
    SvfResponse {
        filter_type,
        resonance: resonance_for_q(q),
    }
    .at(SvfResponse::analog_s(freq, CUTOFF))
    .norm()
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
        for q in [ZERO_RESONANCE_Q, 1.0, 4.0] {
            for freq in [250.0, 1_000.0, 4_000.0] {
                assert_close(
                    measured_gain(filter_type, q, freq),
                    predicted_gain(filter_type, q, freq),
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
            (analog_gain(filter_type, ZERO_RESONANCE_Q, 0.0) - 1.0).abs() < 1e-6,
            "{filter_type:?}"
        );
    }

    for filter_type in [
        SvfType::HighPass12,
        SvfType::HighPass18,
        SvfType::HighPass24,
    ] {
        assert!(
            analog_gain(filter_type, ZERO_RESONANCE_Q, 0.0) < 1e-6,
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
fn q_from_resonance_follows_cubic_curve() {
    assert!((q_from_resonance(0.0) - ZERO_RESONANCE_Q).abs() < 1e-6);

    let resonance: Sample = 0.5;
    let expected =
        ZERO_RESONANCE_Q + (MAX_RESONANCE_Q - ZERO_RESONANCE_Q) * resonance * resonance * resonance;

    assert!((q_from_resonance(resonance) - expected).abs() < 1e-6);
    assert!((q_from_resonance(1.0) - MAX_RESONANCE_Q).abs() < 1e-6);
}

#[test]
fn zero_resonance_lowpass_is_minus_6db_at_cutoff() {
    // |H(fc)| of a 2-pole low-pass equals its Q, so Q = 0.5 is −6 dB.
    let gain = analog_gain(SvfType::LowPass12, ZERO_RESONANCE_Q, CUTOFF);

    assert!((gain - ZERO_RESONANCE_Q).abs() < 1e-5, "{gain}");
}

#[test]
fn steeper_types_roll_off_faster() {
    let far = 8.0 * CUTOFF;
    let gain = |filter_type| analog_gain(filter_type, ZERO_RESONANCE_Q, far);

    assert!(gain(SvfType::LowPass12) > gain(SvfType::LowPass18));
    assert!(gain(SvfType::LowPass18) > gain(SvfType::LowPass24));
    assert!(gain(SvfType::BandPass6) > gain(SvfType::BandPass12));

    let near = CUTOFF / 8.0;
    let gain = |filter_type| analog_gain(filter_type, ZERO_RESONANCE_Q, near);

    assert!(gain(SvfType::HighPass12) > gain(SvfType::HighPass18));
    assert!(gain(SvfType::HighPass18) > gain(SvfType::HighPass24));
}

#[test]
fn high_resonance_impulse_decays_and_stays_finite() {
    for filter_type in SvfType::ALL {
        let (g, k) = g_and_k(CUTOFF, 16.0);
        let mut state = SvfState::new(filter_type);
        let mut peak_early = 0.0f32;
        let mut peak_late = 0.0f32;

        for n in 0..SAMPLE_RATE as usize {
            let x = if n == 0 { 1.0 } else { 0.0 };
            let y = state.tick(g, k, x);

            assert!(y.is_finite(), "{filter_type:?} at {n}");

            if n < 2_000 {
                peak_early = peak_early.max(y.abs());
            } else if n >= 40_000 {
                peak_late = peak_late.max(y.abs());
            }
        }

        assert!(state.is_finite());
        assert!(
            peak_late < peak_early * 0.01,
            "{filter_type:?}: early {peak_early} late {peak_late}"
        );
    }
}

#[test]
fn reset_clears_memory() {
    let (g, k) = g_and_k(CUTOFF, 2.0);
    let mut state = SvfState::new(SvfType::LowPass24);

    for _ in 0..100 {
        state.tick(g, k, 1.0);
    }

    state.reset();

    assert_eq!(state.tick(g, k, 0.0), 0.0);
}

#[test]
fn types_round_trip_through_serde() {
    for filter_type in SvfType::ALL {
        let json = serde_json::to_string(&filter_type).unwrap();
        let back: SvfType = serde_json::from_str(&json).unwrap();

        assert_eq!(back, filter_type);
    }
}
