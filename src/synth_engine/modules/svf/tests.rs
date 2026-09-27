use super::*;
use crate::utils::{note_to_pitch, pitch_to_freq};

const SAMPLE_RATE: Sample = 48_000.0;

fn assert_approx(a: Sample, b: Sample) {
    assert!((a - b).abs() < 1e-3 * b.abs().max(1.0), "{a} ≈ {b}");
}

#[test]
fn zero_cutoff_without_keytrack_is_c4_for_any_note() {
    let c4_freq = pitch_to_freq(C4_PITCH);

    for note in [36u8, 60, 84] {
        assert_approx(
            cutoff_freq(0.0, 0.0, note_to_pitch(note as Sample), SAMPLE_RATE),
            c4_freq,
        );
    }
}

#[test]
fn full_keytrack_follows_the_note() {
    for note in [36u8, 60, 84] {
        let pitch = note_to_pitch(note as Sample);

        // Cutoff of one octave sits an octave above the played note.
        assert_approx(
            cutoff_freq(1.0, 1.0, pitch, SAMPLE_RATE),
            2.0 * pitch_to_freq(pitch),
        );
    }
}

#[test]
fn partial_keytrack_interpolates_in_octaves() {
    let pitch = note_to_pitch(72.0); // C5, one octave above C4

    assert_approx(cutoff_pitch(0.0, 0.5, pitch), C4_PITCH + 0.5);
    assert_approx(cutoff_pitch(2.0, 0.25, pitch), C4_PITCH + 2.25);
}

#[test]
fn cutoff_freq_is_clamped_to_tunable_range() {
    assert_approx(
        cutoff_freq(MIN_CUTOFF, 1.0, note_to_pitch(0.0), SAMPLE_RATE),
        MIN_CUTOFF_FREQ,
    );
    assert_approx(
        cutoff_freq(MAX_CUTOFF, 1.0, note_to_pitch(127.0), SAMPLE_RATE),
        SAMPLE_RATE * MAX_CUTOFF_RATIO,
    );
    // Out-of-range octave values are clamped before key tracking is applied.
    assert_approx(
        cutoff_pitch(MAX_CUTOFF + 10.0, 0.0, C4_PITCH),
        C4_PITCH + MAX_CUTOFF,
    );
}

#[test]
fn config_round_trips_and_defaults_missing_keytrack() {
    let config = SvfConfig {
        id: 7,
        filter_type: SvfType::BandPass12,
        keytrack: 0.5,
        cutoff: StereoSample::new(1.0, 2.0),
        resonance: StereoSample::new(-0.5, 0.5),
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
fn saturation_is_bounded_and_gain_scaled() {
    assert!(saturate(100.0, 24.0).abs() <= 1.0);
    assert!(saturate(-100.0, 24.0).abs() <= 1.0);
    // -60 dB into tanh is effectively linear.
    assert_approx(saturate(0.5, -60.0), 0.5 * 1e-3);
    // More drive → more output for the same small input.
    assert!(saturate(0.1, 12.0) > saturate(0.1, 0.0));
}
