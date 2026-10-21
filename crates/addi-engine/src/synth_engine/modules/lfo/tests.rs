use addi_dsp::segments::{Segment, Segments};

use super::*;

type TestLfo = Lfo<stub::Links>;

fn assert_points_eq(actual: &[Segment], expected: &[Segment]) {
    assert_eq!(actual.len(), expected.len());
    for (a, e) in actual.iter().zip(expected) {
        assert_eq!(a.x, e.x);
        assert_eq!(a.y, e.y);
        assert_eq!(a.power, e.power);
        assert_eq!(a.smooth, e.smooth);
    }
}

fn test_square() -> Segments {
    Segments::from_points(
        &[(0.0, 0.0), (0.0, 1.0), (0.5, 1.0), (0.5, 0.0), (1.0, 0.0)],
        false,
    )
}

fn test_sine() -> Segments {
    let mut segments = Segments::from_points(&[(0.0, 0.0), (0.5, 1.0), (1.0, 0.0)], false);

    for index in 0..segments.num_segments() {
        segments.set_smooth(index, true);
    }

    segments
}

fn make_lfo(segments: Segments) -> TestLfo {
    Lfo::from_config(
        &LfoConfig {
            id: 1,
            segments,
            ..LfoConfig::default()
        },
        16,
    )
}

fn phase_at(index: usize, fraction: Smp) -> Phase {
    let shift = 32 - WAVEFORM_BITS;
    let bits = ((index as u32) << shift).wrapping_add((fraction * (1u32 << shift) as Smp) as u32);

    Phase::from_bits(bits)
}

#[test]
fn table_knots_match_the_segments() {
    let lfo = make_lfo(test_sine());

    let mut rendered = vec![0.0; WAVEFORM_SIZE];
    lfo.get_config().segments.render(&mut rendered);

    for index in [0, 1, 100, 1024, 2047] {
        let baked = lfo.wave[WAVEFORM_PAD_LEFT + index];

        assert_eq!(baked, rendered[index]);
        assert_eq!(
            TestLfo::read_wave(&lfo.wave, phase_at(index, 0.0)),
            baked,
            "index {index}"
        );
    }
}

#[test]
fn sine_midpoints_follow_catmull_rom() {
    let lfo = make_lfo(test_sine());

    let mut fine = vec![0.0; WAVEFORM_SIZE * 2];
    lfo.get_config().segments.render(&mut fine);

    for index in [0, 10, 500, 2046] {
        let got = TestLfo::read_wave(&lfo.wave, phase_at(index, 0.5));
        let expected = fine[index * 2 + 1];

        assert!(
            (got - expected).abs() < 1e-4,
            "index {index}: {got} vs {expected}"
        );
    }
}

#[test]
fn playback_applies_phase_shift() {
    let lfo = make_lfo(test_sine());
    let index = 0;
    let shift = 0.125;
    let shift_index = (shift * WAVEFORM_SIZE as Smp) as usize;
    let read = TestLfo::read_wave(&lfo.wave, phase_at(index, 0.0).add_normalized(shift));
    let expected = lfo.wave[WAVEFORM_PAD_LEFT + shift_index];

    assert_eq!(read, expected);
}

#[test]
fn set_segments_rebakes_the_table() {
    let mut lfo = make_lfo(Segments::default());
    let before = lfo.wave[WAVEFORM_PAD_LEFT + 100];

    lfo.set_segments(test_square());

    assert_ne!(lfo.wave[WAVEFORM_PAD_LEFT + 100], before);
    assert_points_eq(
        lfo.get_config().segments.segments(),
        test_square().segments(),
    );
    assert_eq!(lfo.wave[WAVEFORM_PAD_LEFT + 100], 1.0);
    assert_eq!(lfo.wave[WAVEFORM_PAD_LEFT + 1500], 0.0);

    lfo.set_segments(test_square());

    assert_points_eq(
        lfo.get_config().segments.segments(),
        test_square().segments(),
    );
}

#[test]
fn from_config_sanitizes_params() {
    let lfo = TestLfo::from_config(
        &LfoConfig {
            frequency: StereoSample::new(1_000.0, Smp::NAN),
            phase_shift: StereoSample::new(-4.0, 0.25),
            segments: Segments::from_points(&[(0.0, -2.0), (1.0, 3.0)], false),
            ..LfoConfig::default()
        },
        16,
    );
    let config = lfo.get_config();

    assert_eq!(config.frequency, StereoSample::new(100.0, 0.0));
    assert_eq!(config.phase_shift, StereoSample::new(-1.0, 0.25));
    assert_eq!(config.segments.segment(0).y, 0.0);
    assert_eq!(config.segments.segment(1).y, 1.0);
}

#[test]
fn missing_timebase_defaults_to_hertz() {
    let config: LfoConfig = serde_json::from_str(r#"{"id":1}"#).unwrap();

    assert_eq!(config.timebase, Timebase::Hertz);
    assert_eq!(config.frequency, StereoSample::splat(1.0));
}

#[test]
fn wrapped_read_stays_finite() {
    let lfo = make_lfo(test_square());
    let y = TestLfo::read_wave(&lfo.wave, Phase::from_bits(u32::MAX));

    assert!(y.is_finite());
}
