use addi_dsp::segments::Segments;

use super::stub::Links;
use super::{ModMap, ModMapConfig, SMOOTH_K, SvfStage, map_control, read_table, smoothing_g};

type TestMap = ModMap<Links>;

#[test]
fn identity_table_matches_the_input() {
    let map = TestMap::new(1, 16);

    for x in [0.0, 0.25, 0.5, 0.75, 1.0] {
        let y = read_table(&map.table, x);

        assert!((y - x).abs() < 1e-5, "{x} -> {y}");
    }
}

#[test]
fn curved_table_follows_the_segments() {
    let mut config = ModMapConfig::default();
    config.segments = Segments::from_points(&[(0.0, 0.0), (0.3, 0.8), (1.0, 0.2)], true);
    config.segments.set_power(0, 2.0);

    let map = TestMap::from_config(&config, 32);
    let mut worst = 0.0f32;

    for i in 0..=32 {
        let x = i as f32 / 32.0;
        let expected = config.segments.value_at(x, None, None);
        let y = read_table(&map.table, x);

        worst = worst.max((y - expected).abs());
    }

    assert!(worst < 1e-3, "worst {worst}");
}

#[test]
fn values_outside_the_unit_interval_clamp_to_the_ends() {
    let map = TestMap::new(1, 16);
    let at_start = read_table(&map.table, 0.0);
    let at_end = read_table(&map.table, 1.0);

    assert!((read_table(&map.table, -2.0) - at_start).abs() < 1e-6);
    assert!((read_table(&map.table, 3.0) - at_end).abs() < 1e-6);
}

#[test]
fn amount_scales_before_the_bipolar_fold() {
    assert!((map_control(0.5, 1.0, false) - 0.5).abs() < 1e-6);
    assert!((map_control(0.5, 0.5, false) - 0.25).abs() < 1e-6);
    assert!((map_control(0.0, 1.0, true) - -1.0).abs() < 1e-6);
    assert!((map_control(1.0, 1.0, true) - 1.0).abs() < 1e-6);
    assert!(map_control(0.5, 1.0, true).abs() < 1e-6);
    assert!((map_control(1.0, 0.0, true) - -1.0).abs() < 1e-6);
    assert!((map_control(1.0, 1.5, false) - 1.0).abs() < 1e-6);
}

#[test]
fn missing_config_fields_default_to_a_pass_through() {
    let config: ModMapConfig = serde_json::from_str(r#"{"id":4}"#).unwrap();

    assert_eq!(config.id, 4);
    assert_eq!(config.amount, 1.0.into());
    assert!(!config.bipolar);
    assert_eq!(config.smooth, 0.0);
    assert!((config.segments.value_at(0.0, None, None)).abs() < 1e-6);
    assert!((config.segments.value_at(1.0, None, None) - 1.0).abs() < 1e-6);
}

#[test]
fn smooth_stage_approaches_a_step_without_overshoot() {
    let mut stage = SvfStage::default();
    let g = smoothing_g(0.01, 48_000.0);

    assert!((SMOOTH_K - 2.0).abs() < 1e-6);

    stage.reset(0.0);

    let mut prev = 0.0;

    for _ in 0..4_000 {
        let y = stage.tick(g, 1.0);

        assert!(y + 1e-4 >= prev, "{y} after {prev}");
        assert!(y <= 1.0 + 1e-4, "{y}");
        prev = y;
    }

    assert!(prev > 0.5, "{prev}");
}

#[test]
fn shorter_smooth_time_raises_the_cutoff() {
    let slow = smoothing_g(0.05, 48_000.0);
    let fast = smoothing_g(0.001, 48_000.0);

    assert!(fast > slow);
}
