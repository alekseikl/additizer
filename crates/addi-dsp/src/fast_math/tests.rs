use crate::{
    SIMD_LANES, WideSmp,
    math::{db_to_gain_fast, pitch_to_freq},
};

fn wide(values: &[f32]) -> WideSmp {
    let mut lanes = [0.0; SIMD_LANES];
    lanes[..values.len()].copy_from_slice(values);

    WideSmp::new(lanes)
}

use super::{
    db_to_gain_fast_wide, fast_exp2_wide, fast_sin_pi_wide, fast_tan_pi_wide, pitch_to_freq_wide,
};

fn fast_exp2(x: f32) -> f32 {
    fast_exp2_wide(WideSmp::splat(x)).to_array()[0]
}

#[test]
fn fast_exp2_is_accurate_across_range() {
    // Dense sweep over the range that matters for pitch.
    let mut worst = 0.0f32;

    for i in -40_000..=40_000 {
        let x = i as f32 / 400.0; // [-100, 100] in 1/400 octave steps
        let exact = x.exp2();
        let rel = ((fast_exp2(x) - exact) / exact).abs();
        worst = worst.max(rel);
    }

    assert!(worst < 1.5e-7, "max relative error {worst}");
}

#[test]
fn fast_exp2_lanes_are_independent() {
    let out = fast_exp2_wide(wide(&[-3.0, 0.5, 2.25, 10.0])).to_array();

    for (x, y) in [-3.0f32, 0.5, 2.25, 10.0].into_iter().zip(out) {
        assert!(((y - x.exp2()) / x.exp2()).abs() < 1.5e-7, "2^{x} = {y}");
    }
}

#[test]
fn fast_exp2_hits_exact_powers_of_two() {
    for e in -125..=125 {
        assert_eq!(fast_exp2(e as f32), (e as f32).exp2(), "2^{e}");
    }
}

#[test]
fn fast_exp2_clamps_out_of_range_inputs() {
    assert_eq!(fast_exp2(1000.0), fast_exp2(125.0));
    assert_eq!(fast_exp2(-1000.0), fast_exp2(-125.0));
    assert!(fast_exp2(125.0).is_finite());
    assert!(fast_exp2(-125.0).is_normal());
}

fn fast_tan_pi(x: f32) -> f32 {
    fast_tan_pi_wide(WideSmp::splat(x)).to_array()[0]
}

#[test]
fn fast_tan_pi_matches_libm_tan() {
    let mut worst_low = 0.0f32;
    let mut worst_high = 0.0f32;

    for i in 0..=20_000 {
        let ratio = 0.499 * i as f32 / 20_000.0;
        let approx = fast_tan_pi(ratio);
        let exact = (ratio * std::f32::consts::PI).tan();
        let rel = if exact == 0.0 {
            approx.abs()
        } else {
            ((approx - exact) / exact).abs()
        };
        if ratio <= 0.25 {
            worst_low = worst_low.max(rel);
        } else {
            worst_high = worst_high.max(rel);
        }
    }

    assert!(
        worst_low < 2e-7,
        "max relative error below 0.25: {worst_low}"
    );
    assert!(
        worst_high < 5e-5,
        "max relative error above 0.25: {worst_high}"
    );
}

fn fast_sin_pi(x: f32) -> f32 {
    fast_sin_pi_wide(WideSmp::splat(x)).to_array()[0]
}

#[test]
fn fast_sin_pi_matches_libm_sin() {
    let mut worst = 0.0f32;

    for i in -20_000..=20_000 {
        let ratio = 0.5 * i as f32 / 20_000.0;
        let x = ratio * std::f32::consts::PI;
        let err = (fast_sin_pi(ratio) - x.sin()).abs();
        worst = worst.max(err);
    }

    assert!(worst < 2e-7, "max absolute error {worst}");
    assert_eq!(fast_sin_pi(0.0), 0.0);
    assert_eq!(fast_sin_pi(0.5), 1.0);
    assert_eq!(fast_sin_pi(-0.5), -1.0);
}

#[test]
fn fast_sin_pi_lanes_are_independent() {
    let ratios = [-0.5f32, -0.2, 0.1, 0.5];
    let out = fast_sin_pi_wide(wide(&ratios)).to_array();

    for (ratio, y) in ratios.into_iter().zip(out) {
        assert_eq!(y, fast_sin_pi(ratio), "sin(π * {ratio})");
    }
}

#[test]
fn fast_tan_pi_lanes_are_independent() {
    let ratios = [0.01f32, 0.2, 0.4, 0.499];
    let out = fast_tan_pi_wide(wide(&ratios)).to_array();

    for (ratio, y) in ratios.into_iter().zip(out) {
        let exact = fast_tan_pi(ratio);
        assert_eq!(y, exact, "tan(π * {ratio})");
    }
}

#[test]
fn db_to_gain_fast_wide_matches_scalar_over_drive_range() {
    let lanes = [-60.0, -12.0, 0.0, 24.0];
    let out = db_to_gain_fast_wide(wide(&lanes)).to_array();

    for (dbs, gain) in lanes.into_iter().zip(out) {
        let exact = db_to_gain_fast(dbs);

        assert!(
            ((gain - exact) / exact).abs() < 5e-7,
            "{dbs} dB: {gain} vs {exact}"
        );
    }
}

#[test]
fn fast_pitch_to_freq_matches_pitch_to_freq() {
    for i in -120..=120 {
        let pitch = i as f32 / 12.0;
        let exact = pitch_to_freq(pitch);
        let fast = pitch_to_freq_wide(WideSmp::splat(pitch)).to_array()[0];

        assert!(
            ((fast - exact) / exact).abs() < 2e-7,
            "pitch {pitch}: {fast} vs {exact}"
        );
    }
}
