use super::{MAX_RESONANCE_Q, ZERO_RESONANCE_Q, *};
use crate::utils::db_to_gain_fast;

fn assert_approx(a: Sample, b: Sample) {
    assert!((a - b).abs() < 1e-3 * b.abs().max(1.0), "{a} ≈ {b}");
}

#[test]
fn config_round_trips_and_defaults_missing_keytrack() {
    let config = SvfConfig {
        id: 7,
        filter_type: SvfType::BandPass12,
        keytrack: 0.5,
        cutoff: StereoSample::new(1.0, 2.0),
        resonance: StereoSample::new(0.2, 0.5),
        drive: StereoSample::new(6.0, -6.0),
    };
    let mut json = serde_json::to_value(&config).unwrap();
    let back: SvfConfig = serde_json::from_value(json.clone()).unwrap();

    assert_eq!(back.id, 7);
    assert_eq!(back.filter_type, SvfType::BandPass12);
    assert_eq!(back.keytrack, 0.5);
    assert_eq!(back.cutoff, config.cutoff);
    assert_eq!(back.resonance, config.resonance);
    assert_eq!(back.drive, config.drive);

    json.as_object_mut().unwrap().remove("keytrack");

    let back: SvfConfig = serde_json::from_value(json).unwrap();

    assert_eq!(back.keytrack, 0.0);
}

#[test]
fn module_config_round_trips_through_get_config() {
    let config = SvfConfig {
        id: 3,
        filter_type: SvfType::HighPass18,
        keytrack: 1.0,
        cutoff: StereoSample::new(-1.0, 4.0),
        resonance: StereoSample::new(0.25, 0.75),
        drive: StereoSample::new(12.0, 0.0),
    };
    let module = Svf::from_config(&config);
    let back = module.get_config();

    assert_eq!(back.id, 3);
    assert_eq!(back.filter_type, SvfType::HighPass18);
    assert_eq!(back.keytrack, 1.0);
    assert_eq!(back.cutoff, config.cutoff);
    assert_eq!(back.resonance, config.resonance);
    assert_eq!(back.drive, config.drive);
}

#[test]
fn q_from_resonance_follows_cubic_curve() {
    assert!((q_from_resonance(0.0) - ZERO_RESONANCE_Q).abs() < 1e-6);

    let resonance: Sample = 0.5;
    let expected =
        ZERO_RESONANCE_Q + (MAX_RESONANCE_Q - ZERO_RESONANCE_Q) * resonance * resonance * resonance;

    assert!((q_from_resonance(resonance) - expected).abs() < 1e-6);
    assert!((q_from_resonance(1.0) - MAX_RESONANCE_Q).abs() < 1e-6);
}

#[test]
fn resonance_is_clamped_to_zero_and_one() {
    let module = Svf::from_config(&SvfConfig {
        resonance: StereoSample::new(-0.5, 1.5),
        ..SvfConfig::default()
    });
    let back = module.get_config();

    assert_eq!(back.resonance, StereoSample::new(0.0, 1.0));
}

#[test]
fn drive_gain_is_clamped_db() {
    let gain = |drive_db: Sample| db_to_gain_fast(drive_db.clamp(MIN_DRIVE, MAX_DRIVE));
    let quiet = gain(-60.0);
    let unity = gain(0.0);
    let hot = gain(12.0);
    let max = gain(MAX_DRIVE);

    assert_approx(quiet, 1e-3);
    assert_approx(unity, 1.0);
    assert!(hot > unity);
    assert_approx(gain(MAX_DRIVE + 24.0), max);
}
