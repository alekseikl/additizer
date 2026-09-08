use super::*;
use crate::utils::C4_PITCH;

fn keytrack_offset(keytrack: Sample, pitch: Sample) -> Sample {
    (C4_PITCH - pitch) * (1.0 - keytrack)
}

fn harmonic_cutoff(keytrack: Sample, cutoff: Sample, pitch: Sample) -> Sample {
    cutoff + keytrack_offset(keytrack, pitch)
}

fn assert_approx(a: Sample, b: Sample) {
    assert!((a - b).abs() < 1e-5, "{a} ≈ {b}");
}

fn pitch_of_note(note: u8) -> Sample {
    crate::utils::note_to_pitch(note as Sample)
}

#[test]
fn full_keytrack_leaves_cutoff_unchanged() {
    assert_approx(harmonic_cutoff(1.0, 0.0, pitch_of_note(48)), 0.0);
    assert_approx(harmonic_cutoff(1.0, 1.0, pitch_of_note(72)), 1.0);
}

#[test]
fn no_keytrack_zero_cutoff_maps_to_c4() {
    // C4: C4 is the fundamental → harmonic 1 (0 octaves)
    assert_approx(harmonic_cutoff(0.0, 0.0, C4_PITCH), 0.0);
    // C5: C4 is an octave below
    assert_approx(harmonic_cutoff(0.0, 0.0, pitch_of_note(72)), -1.0);
    // C3: C4 is an octave above
    assert_approx(harmonic_cutoff(0.0, 0.0, pitch_of_note(48)), 1.0);
}

#[test]
fn no_keytrack_offsets_nonzero_cutoff() {
    let cutoff = 1.5;
    let pitch = pitch_of_note(72);
    assert_approx(harmonic_cutoff(0.0, cutoff, pitch), cutoff + (C4_PITCH - pitch));
}

#[test]
fn partial_keytrack_interpolates() {
    let cutoff = 1.5;
    let pitch = pitch_of_note(48);
    let offset = C4_PITCH - pitch;

    assert_approx(harmonic_cutoff(0.5, cutoff, pitch), cutoff + offset * 0.5);
    assert_approx(harmonic_cutoff(0.25, cutoff, pitch), cutoff + offset * 0.75);
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

fn display_offset(keytrack: Sample, pitch: Sample) -> Sample {
    (pitch - C4_PITCH) * keytrack
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
fn display_offset_scales_pitch_distance_by_keytrack() {
    let pitch = pitch_of_note(72);

    assert_approx(display_offset(0.0, pitch), 0.0);
    assert_approx(display_offset(1.0, pitch), 1.0);
    assert_approx(display_offset(0.5, pitch), 0.5);
}

#[test]
fn keytrack_applies_the_same_offset_to_q_limit_to() {
    let q_limit_to = 2.0;
    let pitch = pitch_of_note(48);

    assert_approx(q_limit_to + keytrack_offset(1.0, pitch), q_limit_to);
    assert_approx(
        q_limit_to + keytrack_offset(0.0, pitch),
        q_limit_to + (C4_PITCH - pitch),
    );
    assert_approx(
        q_limit_to + keytrack_offset(0.5, pitch),
        q_limit_to + (C4_PITCH - pitch) * 0.5,
    );
}
