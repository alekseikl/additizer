use super::*;

fn approx(a: Sample, b: Sample) -> bool {
    (a - b).abs() < 1e-4
}

#[test]
fn parse_octaves_st() {
    let value = Units::Octaves(OctavesDisplay::Semitones)
        .parse_one("12.5 st")
        .unwrap();
    assert!(approx(value, 12.5 / 12.0));
}

#[test]
fn parse_octaves_cents() {
    let value = Units::Octaves(OctavesDisplay::Semitones)
        .parse_one("50 cents")
        .unwrap();
    assert!(approx(value, 50.0 / 1_200.0));
}

#[test]
fn parse_octaves_hz_from_c4() {
    let value = Units::Octaves(OctavesDisplay::Frequency)
        .parse_one("440")
        .unwrap();
    assert!(approx(value, 0.75));

    let value = Units::Octaves(OctavesDisplay::Frequency)
        .parse_one("2.5 kHz")
        .unwrap();
    assert!(approx(value, freq_to_c4_pitch(2_500.0)));
}

#[test]
fn parse_time_stereo_mixed_units() {
    let value = Units::Time.parse("450 ms, 1.4 s", true).unwrap();
    assert!(approx(value.left(), 0.45));
    assert!(approx(value.right(), 1.4));
}

#[test]
fn format_db_zero_has_no_sign() {
    assert_eq!(Units::Db.format(0.0), "0.0 dB");
    assert_eq!(Units::Db.format(-0.0), "0.0 dB");
    assert_eq!(Units::Db.format(0.04), "+0.0 dB");
    assert_eq!(Units::Db.format(-0.04), "-0.0 dB");
    assert_eq!(Units::Db.format(6.0), "+6.0 dB");
    assert_eq!(Units::Db.format(-6.0), "-6.0 dB");
}

#[test]
fn format_rolloff_db_per_octave() {
    assert_eq!(Units::Rolloff.format(24.0), "24.0 dB/oct");
    assert_eq!(Units::Rolloff.format(12.5), "12.5 dB/oct");
    assert_eq!(Units::Rolloff.format_input(24.0), "24");
}

#[test]
fn parse_rolloff_db_per_octave() {
    let value = Units::Rolloff.parse_one("24.0 dB/oct").unwrap();
    assert!(approx(value, 24.0));

    let value = Units::Rolloff.parse_one("12.5").unwrap();
    assert!(approx(value, 12.5));
}

#[test]
fn parse_db_signed() {
    let value = Units::Db.parse_one("+12.5 dB").unwrap();
    assert!(approx(value, 12.5));
}

#[test]
fn parse_normalized_percent() {
    let value = Units::Normalized.parse_one("50%").unwrap();
    assert!(approx(value, 0.5));
}

#[test]
fn parse_frequency_khz() {
    let value = Units::Frequency.parse_one("1.5 kHz").unwrap();
    assert!(approx(value, 1_500.0));
}

#[test]
fn parse_single_value_splats_stereo() {
    let value = Units::Time.parse("450 ms", true).unwrap();
    assert!(approx(value.left(), 0.45));
    assert!(approx(value.right(), 0.45));
}

#[test]
fn parse_rejects_two_values_on_mono() {
    assert!(Units::Time.parse("450 ms, 1.4 s", false).is_none());
}

#[test]
fn parse_rejects_wrong_unit() {
    assert!(Units::Time.parse_one("12.5 st").is_none());
}

#[test]
fn format_input_trims_default_units_and_trailing_zeros() {
    assert_eq!(Units::Normalized.format_input(0.5), "50");
    assert_eq!(Units::Db.format_input(-6.0), "-6");
    assert_eq!(Units::Rolloff.format_input(24.0), "24");
    assert_eq!(
        Units::Octaves(OctavesDisplay::Semitones).format_input(12.5 / 12.0),
        "12.5"
    );
    assert_eq!(
        Units::Octaves(OctavesDisplay::Semitones).format_input(50.0 / 1_200.0),
        "50 cents"
    );
    assert_eq!(
        Units::Octaves(OctavesDisplay::Frequency).format_input(0.75),
        "440"
    );
    assert_eq!(
        Units::Octaves(OctavesDisplay::Frequency).format_input(freq_to_c4_pitch(2_500.0)),
        "2.5 kHz"
    );
    assert_eq!(Units::Frequency.format_input(440.0), "440");
    assert_eq!(Units::Frequency.format_input(2_500.0), "2.5 kHz");
    assert_eq!(Units::Time.format_input(0.004), "4 ms");
    assert_eq!(Units::Time.format_input(0.45), "450 ms");
    assert_eq!(Units::Time.format_input(1.4), "1.4");
}

#[test]
fn format_input_parse_roundtrip() {
    for (units, value) in [
        (Units::Normalized, 0.5),
        (Units::Db, -6.0),
        (Units::Rolloff, 24.0),
        (Units::Rolloff, 12.5),
        (Units::Octaves(OctavesDisplay::Semitones), 12.5 / 12.0),
        (Units::Octaves(OctavesDisplay::Semitones), 50.0 / 1_200.0),
        (Units::Octaves(OctavesDisplay::Frequency), 0.75),
        (
            Units::Octaves(OctavesDisplay::Frequency),
            freq_to_c4_pitch(2_500.0),
        ),
        (Units::Frequency, 440.0),
        (Units::Frequency, 2_500.0),
        (Units::Time, 0.004),
        (Units::Time, 0.45),
        (Units::Time, 1.4),
    ] {
        let parsed = units.parse_one(&units.format_input(value)).unwrap();
        assert!(
            approx(parsed, value),
            "{value} as {:?} formatted {:?}, parsed {parsed}",
            std::mem::discriminant(&units),
            units.format_input(value)
        );
    }
}

#[test]
fn format_parse_roundtrip() {
    for (units, value) in [
        (Units::Normalized, 0.5),
        (Units::Db, -6.0),
        (Units::Rolloff, 24.0),
        (Units::Rolloff, 12.5),
        (Units::Octaves(OctavesDisplay::Semitones), 12.5 / 12.0),
        (Units::Octaves(OctavesDisplay::Semitones), 50.0 / 1_200.0),
        (Units::Octaves(OctavesDisplay::Frequency), 0.75),
        (
            Units::Octaves(OctavesDisplay::Frequency),
            freq_to_c4_pitch(2_500.0),
        ),
        (Units::Frequency, 440.0),
        (Units::Frequency, 2_500.0),
        (Units::Time, 0.004),
        (Units::Time, 0.45),
        (Units::Time, 1.4),
    ] {
        let parsed = units.parse_one(&units.format(value)).unwrap();
        assert!(
            approx(parsed, value),
            "{value} as {:?} formatted {:?}, parsed {parsed}",
            std::mem::discriminant(&units),
            units.format(value)
        );
    }
}
