use super::*;
use crate::utils::from_st;

fn keytrack_offset(keytrack: Sample, note: u8) -> Sample {
    from_st(C4_NOTE as Sample - note as Sample) * (1.0 - keytrack)
}

fn harmonic_cutoff(keytrack: Sample, cutoff: Sample, note: u8) -> Sample {
    cutoff + keytrack_offset(keytrack, note)
}

fn assert_approx(a: Sample, b: Sample) {
    assert!((a - b).abs() < 1e-5, "{a} ≈ {b}");
}

fn c4_offset(note: u8) -> Sample {
    from_st(C4_NOTE as Sample - note as Sample)
}

#[test]
fn full_keytrack_leaves_cutoff_unchanged() {
    assert_approx(harmonic_cutoff(1.0, 0.0, 48), 0.0);
    assert_approx(harmonic_cutoff(1.0, 1.0, 72), 1.0);
}

#[test]
fn no_keytrack_zero_cutoff_maps_to_c4() {
    // C4: C4 is the fundamental → harmonic 1 (0 octaves)
    assert_approx(harmonic_cutoff(0.0, 0.0, C4_NOTE), 0.0);
    // C5: C4 is an octave below
    assert_approx(harmonic_cutoff(0.0, 0.0, 72), from_st(-12.0));
    // C3: C4 is an octave above
    assert_approx(harmonic_cutoff(0.0, 0.0, 48), from_st(12.0));
}

#[test]
fn no_keytrack_offsets_nonzero_cutoff() {
    let cutoff = 1.5;
    assert_approx(harmonic_cutoff(0.0, cutoff, 72), cutoff + c4_offset(72));
}

#[test]
fn partial_keytrack_interpolates() {
    let cutoff = 1.5;
    let note = 48;
    let offset = c4_offset(note);

    assert_approx(harmonic_cutoff(0.5, cutoff, note), cutoff + offset * 0.5);
    assert_approx(harmonic_cutoff(0.25, cutoff, note), cutoff + offset * 0.75);
}

fn config_from_keytrack(value: Option<serde_json::Value>) -> SpectralFilterConfig {
    let mut json = serde_json::to_value(SpectralFilterConfig::default()).unwrap();
    match value {
        Some(value) => json["keytrack"] = value,
        None => {
            json.as_object_mut().unwrap().remove("keytrack");
        }
    }
    serde_json::from_value(json).unwrap()
}

fn display_offset(keytrack: Sample, note: u8) -> Sample {
    from_st(note as Sample - C4_NOTE as Sample) * keytrack
}

#[test]
fn keytrack_deserializes_from_number_and_defaults_when_missing() {
    assert_approx(
        config_from_keytrack(Some(serde_json::json!(0.25))).keytrack,
        0.25,
    );
    assert_approx(config_from_keytrack(None).keytrack, 1.0);
}

#[test]
fn display_offset_scales_note_distance_by_keytrack() {
    let note = 72;

    assert_approx(display_offset(0.0, note), 0.0);
    assert_approx(display_offset(1.0, note), from_st(12.0));
    assert_approx(display_offset(0.5, note), from_st(6.0));
}

#[test]
fn keytrack_applies_the_same_offset_to_q_limit_to() {
    let q_limit_to = 2.0;
    let note = 48;

    assert_approx(q_limit_to + keytrack_offset(1.0, note), q_limit_to);
    assert_approx(
        q_limit_to + keytrack_offset(0.0, note),
        q_limit_to + c4_offset(note),
    );
    assert_approx(
        q_limit_to + keytrack_offset(0.5, note),
        q_limit_to + c4_offset(note) * 0.5,
    );
}
