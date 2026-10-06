use wide::f32x4;

use super::{
    db_to_gain_fast, db_to_gain_fast_x4, fast_exp2_x4, fast_pitch_to_freq_x4, fast_tan_pi_x4,
    map_x4, map_x4_in_place, pitch_to_freq, zip_map_x4,
};

fn fast_exp2(x: f32) -> f32 {
    fast_exp2_x4(f32x4::splat(x)).to_array()[0]
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
    let out = fast_exp2_x4(f32x4::new([-3.0, 0.5, 2.25, 10.0])).to_array();

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
    fast_tan_pi_x4(f32x4::splat(x)).to_array()[0]
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

#[test]
fn fast_tan_pi_lanes_are_independent() {
    let ratios = [0.01f32, 0.2, 0.4, 0.499];
    let out = fast_tan_pi_x4(f32x4::new(ratios)).to_array();

    for (ratio, y) in ratios.into_iter().zip(out) {
        let exact = fast_tan_pi(ratio);
        assert_eq!(y, exact, "tan(π * {ratio})");
    }
}

#[test]
fn map_x4_chunks_and_zeroes_remainder() {
    let mut out = [0.0; 5];
    map_x4(&[1.0, 2.0, 3.0, 4.0, 5.0], &mut out, |lanes| {
        lanes + f32x4::splat(10.0)
    });
    assert_eq!(out, [11.0, 12.0, 13.0, 14.0, 15.0]);

    let mut out = [0.0; 2];
    map_x4(&[1.0, 2.0], &mut out, |lanes| {
        f32x4::splat(lanes.reduce_add())
    });
    assert_eq!(out, [3.0, 3.0]);

    let mut out = [];
    map_x4(&[], &mut out, |lanes| lanes);
}

#[test]
fn zip_map_x4_pairs_lanes_and_zeroes_remainder() {
    let mut out = [0.0; 5];
    zip_map_x4(
        &[1.0, 2.0, 3.0, 4.0, 5.0],
        &[10.0, 20.0, 30.0, 40.0, 50.0],
        &mut out,
        |a, b| a + b,
    );
    assert_eq!(out, [11.0, 22.0, 33.0, 44.0, 55.0]);

    let mut out = [0.0; 2];
    zip_map_x4(&[1.0, 2.0], &[3.0, 4.0], &mut out, |a, b| {
        f32x4::splat((a + b).reduce_add())
    });
    assert_eq!(out, [10.0, 10.0]);

    let mut out = [];
    zip_map_x4(&[], &[], &mut out, |a, _b| a);
}

#[test]
fn map_x4_in_place_matches_map_x4() {
    let samples = [1.0, 2.0, 3.0, 4.0, 5.0];
    let mut separate = [0.0; 5];
    let mut inplace = samples;

    let map = |lanes: f32x4| lanes + f32x4::splat(10.0);

    map_x4(&samples, &mut separate, map);
    map_x4_in_place(&mut inplace, map);

    assert_eq!(inplace, separate);
}

#[test]
fn db_to_gain_fast_x4_matches_scalar_over_drive_range() {
    let lanes = [-60.0, -12.0, 0.0, 24.0];
    let out = db_to_gain_fast_x4(f32x4::new(lanes)).to_array();

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
        let fast = fast_pitch_to_freq_x4(f32x4::splat(pitch)).to_array()[0];

        assert!(
            ((fast - exact) / exact).abs() < 2e-7,
            "pitch {pitch}: {fast} vs {exact}"
        );
    }
}
