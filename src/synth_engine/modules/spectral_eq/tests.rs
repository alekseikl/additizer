use super::*;
use crate::synth_engine::{spectral_filter::q_from_resonance, synth_module::SynthModule};

#[test]
fn from_config_keeps_at_most_16_filters() {
    let config = SpectralEqConfig {
        filters: vec![EqFilter::default(); MAX_EQ_FILTERS + 4],
        ..SpectralEqConfig::default()
    };

    let eq = SpectralEq::from_config(&config);

    assert_eq!(eq.get_config().filters.len(), MAX_EQ_FILTERS);
}

#[test]
fn old_resonance_deserializes_as_q() {
    let json = serde_json::json!({
        "filter_type": "Peaking",
        "cutoff_hz": 440.0,
        "resonance": 0.5,
        "drive": -3.0,
    });

    let filter: EqFilter = serde_json::from_value(json).unwrap();

    assert!((filter.q - q_from_resonance(0.5)).abs() < 1e-6);
    assert_eq!(filter.cutoff_hz, 440.0);
    assert_eq!(filter.drive, -3.0);
}

#[test]
fn q_field_is_kept_when_present() {
    let json = serde_json::json!({
        "filter_type": "LowPass12",
        "cutoff_hz": 1000.0,
        "q": 2.5,
        "resonance": 1.0,
        "drive": 0.0,
    });

    let filter: EqFilter = serde_json::from_value(json).unwrap();

    assert_eq!(filter.q, 2.5);
}

#[test]
fn q_round_trips_and_is_clamped() {
    let filter = EqFilter {
        q: 4.0,
        ..EqFilter::default()
    };
    let json = serde_json::to_value(filter).unwrap();
    assert!(json.get("resonance").is_none());

    let decoded: EqFilter = serde_json::from_value(json).unwrap();
    assert_eq!(decoded.q, 4.0);
    assert_eq!(filter.sanitized().q, 4.0);
    assert_eq!(EqFilter { q: 100.0, ..filter }.sanitized().q, MAX_Q);
    assert_eq!(EqFilter { q: 0.0, ..filter }.sanitized().q, MIN_Q);
}

#[test]
fn move_filter_reorders_bands() {
    let bands = [100.0, 1_000.0, 8_000.0].map(|cutoff_hz| EqFilter {
        cutoff_hz,
        ..EqFilter::default()
    });
    let mut eq = SpectralEq::from_config(&SpectralEqConfig {
        filters: bands.to_vec(),
        ..SpectralEqConfig::default()
    });
    let mut bridge = SpectralEqUiBridge::try_new(&mut eq).unwrap();

    bridge.move_filter(0, 0);
    bridge.move_filter(3, 0);
    eq.process_ui_events();
    assert_eq!(cutoffs(&eq.get_config().filters), [100.0, 1_000.0, 8_000.0]);

    bridge.move_filter(2, 0);
    eq.process_ui_events();
    assert_eq!(cutoffs(&bridge.config().filters), [8_000.0, 100.0, 1_000.0]);
    assert_eq!(cutoffs(&eq.get_config().filters), [8_000.0, 100.0, 1_000.0]);

    bridge.move_filter(0, 1);
    eq.process_ui_events();
    assert_eq!(cutoffs(&eq.get_config().filters), [100.0, 8_000.0, 1_000.0]);
}

fn cutoffs(filters: &[EqFilter]) -> [f32; 3] {
    [
        filters[0].cutoff_hz,
        filters[1].cutoff_hz,
        filters[2].cutoff_hz,
    ]
}
