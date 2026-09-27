use std::f32::consts::PI;

use super::*;

fn assert_close(actual: Sample, expected: Sample) {
    assert!((actual - expected).abs() <= 1e-5, "{actual} vs {expected}");
}

#[test]
fn pads_sample_the_function_outside_the_interval() {
    const INTERVALS: usize = 4;
    let table = LookupTable::<{ INTERVALS + EXTRA_SAMPLES }>::new(|t| t);

    assert_eq!(table.samples, [-0.25, 0.0, 0.25, 0.5, 0.75, 1.0, 1.25, 1.5]);
}

#[test]
fn values_match_at_each_interval_end() {
    const INTERVALS: usize = 16;
    let table = LookupTable::<{ INTERVALS + EXTRA_SAMPLES }>::new(|t| (t * PI).sin());

    for k in 0..=INTERVALS {
        let t = k as Sample / INTERVALS as Sample;

        assert_eq!(table.at(t), (t * PI).sin());
    }
}

#[test]
fn lookup_clamps_t_to_the_unit_interval() {
    const INTERVALS: usize = 4;
    let table = LookupTable::<{ INTERVALS + EXTRA_SAMPLES }>::new(|t| t);

    assert_eq!(table.at(-1.0), 0.0);
    assert_eq!(table.at(2.0), 1.0);
}

#[test]
fn single_interval_of_a_constant_returns_that_value() {
    let table = LookupTable::<{ 1 + EXTRA_SAMPLES }>::new(|_| 3.0);

    for t in [0.0, 0.25, 0.5, 1.0, -1.0, 2.0] {
        assert_eq!(table.at(t), 3.0);
    }
}

#[test]
fn line_is_reproduced_across_the_whole_range() {
    const INTERVALS: usize = 8;
    let table = LookupTable::<{ INTERVALS + EXTRA_SAMPLES }>::new(|t| 2.0 * t - 0.5);

    for i in 0..=64 {
        let t = i as Sample / 64.0;

        assert_close(table.at(t), 2.0 * t - 0.5);
    }
}

#[test]
fn odd_function_keeps_its_slope_at_zero() {
    const INTERVALS: usize = 63;
    let table = LookupTable::<{ INTERVALS + EXTRA_SAMPLES }>::new(|t| (t * PI).sin());
    let t = 0.5 / INTERVALS as Sample;

    assert_close(table.at(t), (t * PI).sin());
}
