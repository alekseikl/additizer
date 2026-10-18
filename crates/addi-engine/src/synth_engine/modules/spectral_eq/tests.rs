use super::*;

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
fn q_round_trips_and_is_clamped() {
    let filter = EqFilter {
        q: 4.0,
        ..EqFilter::default()
    };
    let json = serde_json::to_value(filter).unwrap();
    let decoded: EqFilter = serde_json::from_value(json).unwrap();
    assert_eq!(decoded.q, 4.0);
    assert_eq!(filter.sanitized().q, 4.0);
    assert_eq!(EqFilter { q: 100.0, ..filter }.sanitized().q, MAX_Q);
    assert_eq!(EqFilter { q: 0.0, ..filter }.sanitized().q, MIN_Q);
}
