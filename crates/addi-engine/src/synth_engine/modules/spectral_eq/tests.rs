use super::*;
use addi_dsp::filters::control::q_from_resonance;

#[test]
fn from_config_keeps_at_most_16_filters() {
    let config = SpectralEqConfig {
        filters: vec![EqFilter::default(); MAX_EQ_FILTERS + 4],
        ..SpectralEqConfig::default()
    };

    let eq = <SpectralEq>::from_config(&config);

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

    assert!((filter.q - q_from_resonance(0.5).resonant).abs() < 1e-6);
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
