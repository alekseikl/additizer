use super::*;

#[test]
fn from_config_keeps_at_most_16_filters() {
    let mut config = SpectralEqConfig::default();
    config.filters = vec![EqFilter::default(); MAX_EQ_FILTERS + 4];

    let eq = SpectralEq::from_config(&config);

    assert_eq!(eq.get_config().filters.len(), MAX_EQ_FILTERS);
}
