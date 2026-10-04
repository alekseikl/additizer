use wide::f32x4;

use super::{fast_exp2_x4, fast_pitch_to_freq_x4, pitch_to_freq};

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
