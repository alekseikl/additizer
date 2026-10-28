use std::f32::consts::{PI, TAU};

use super::{CtrlFilter, MAX_FREQ_RATIO};

const SAMPLE_RATE: f32 = 48_000.0;

#[test]
fn smooth_stage_approaches_a_step_without_overshoot() {
    let mut stage = CtrlFilter::default();
    let coeffs = CtrlFilter::coeffs(0.01, SAMPLE_RATE);

    stage.reset(0.0);

    let mut prev = 0.0;

    for _ in 0..4_000 {
        let y = stage.tick(coeffs.g, 1.0);

        assert!(y + 1e-4 >= prev, "{y} after {prev}");
        assert!(y <= 1.0 + 1e-4, "{y}");
        prev = y;
    }

    assert!(prev > 0.5, "{prev}");
}

#[test]
fn shorter_smooth_time_raises_the_cutoff() {
    let slow = CtrlFilter::coeffs(0.05, SAMPLE_RATE).g;
    let fast = CtrlFilter::coeffs(0.001, SAMPLE_RATE).g;

    assert!(fast > slow);
}

#[test]
fn min_time_is_the_time_constant_at_the_nyquist_limit() {
    let coeffs = CtrlFilter::coeffs(0.01, SAMPLE_RATE);
    let expected = (TAU * MAX_FREQ_RATIO * SAMPLE_RATE).recip();

    assert!((coeffs.min_time - expected).abs() < 1e-12);

    let at_limit = CtrlFilter::coeffs(coeffs.min_time, SAMPLE_RATE);
    let expected_g = (PI * MAX_FREQ_RATIO).tan();

    assert!((at_limit.g - expected_g).abs() / expected_g < 1e-5);

    let below = CtrlFilter::coeffs(coeffs.min_time * 0.5, SAMPLE_RATE);

    assert!((below.g - at_limit.g).abs() < 1e-6);
}
