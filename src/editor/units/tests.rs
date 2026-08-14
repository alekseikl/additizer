use super::*;

fn approx(a: Sample, b: Sample) -> bool {
    (a - b).abs() < 1e-4
}

#[test]
fn parse_octaves_st() {
    let value = Units::Octaves.parse_one("12.5 st").unwrap();
    assert!(approx(value, 12.5 / 12.0));
}

#[test]
fn parse_octaves_cents() {
    let value = Units::Octaves.parse_one("50 cents").unwrap();
    assert!(approx(value, 50.0 / 1_200.0));
}

#[test]
fn parse_time_stereo_mixed_units() {
    let value = Units::Time.parse("450 ms, 1.4 s", true).unwrap();
    assert!(approx(value.left(), 0.45));
    assert!(approx(value.right(), 1.4));
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
    assert_eq!(Units::Octaves.format_input(12.5 / 12.0), "12.5");
    assert_eq!(Units::Octaves.format_input(50.0 / 1_200.0), "50 cents");
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
        (Units::Octaves, 12.5 / 12.0),
        (Units::Octaves, 50.0 / 1_200.0),
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
        (Units::Octaves, 12.5 / 12.0),
        (Units::Octaves, 50.0 / 1_200.0),
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
