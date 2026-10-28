use addi_dsp::segments::Segments;

use super::stub::Links;
use super::{ModMap, ModMapConfig, read_table};

type TestMap = ModMap<Links>;

#[test]
fn straight_ramp_skips_the_map_table() {
    let map = TestMap::new(1, 16);

    assert!(!map.remap);
    assert!(map.table.iter().all(|sample| *sample == 0.0));
}

#[test]
fn extra_diagonal_points_render_a_table() {
    let mut map = TestMap::new(1, 16);

    map.set_segments(Segments::from_points(
        &[(0.0, 0.0), (0.4, 0.4), (1.0, 1.0)],
        false,
    ));

    assert!(map.remap);
    assert!(map.table.iter().any(|sample| *sample != 0.0));
}

#[test]
fn eased_or_bent_ramps_render_a_table() {
    let mut eased = TestMap::new(1, 16);
    eased.set_segments(Segments::from_points(&[(0.0, 0.0), (1.0, 1.0)], true));

    assert!(eased.remap);
    assert!(eased.table.iter().any(|sample| *sample != 0.0));

    let mut bent = Segments::from_points(&[(0.0, 0.0), (1.0, 1.0)], false);
    bent.set_power(0, 2.0);

    let mut map = TestMap::new(1, 16);
    map.set_segments(bent);

    assert!(map.remap);
}

#[test]
fn returning_to_a_straight_ramp_stops_mapping() {
    let config = ModMapConfig {
        segments: Segments::from_points(&[(0.0, 0.0), (1.0, 0.25)], false),
        ..ModMapConfig::default()
    };

    let mut map = TestMap::from_config(&config, 16);
    assert!(map.remap);

    map.set_segments(Segments::from_points(&[(0.0, 0.0), (1.0, 1.0)], false));
    assert!(!map.remap);
}

#[test]
fn curved_table_follows_the_segments() {
    let mut config = ModMapConfig {
        segments: Segments::from_points(&[(0.0, 0.0), (0.3, 0.8), (1.0, 0.2)], true),
        ..ModMapConfig::default()
    };
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
    let config = ModMapConfig {
        segments: Segments::from_points(&[(0.0, 0.2), (1.0, 0.8)], false),
        ..ModMapConfig::default()
    };

    let map = TestMap::from_config(&config, 16);
    let at_start = read_table(&map.table, 0.0);
    let at_end = read_table(&map.table, 1.0);

    assert!((at_start - 0.2).abs() < 1e-4);
    assert!((at_end - 0.8).abs() < 1e-4);

    assert!((read_table(&map.table, -2.0) - at_start).abs() < 1e-6);
    assert!((read_table(&map.table, 3.0) - at_end).abs() < 1e-6);
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
