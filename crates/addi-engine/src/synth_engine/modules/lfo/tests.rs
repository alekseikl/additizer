use addi_dsp::segments::Segments;

use super::*;

type TestLfo = Lfo<stub::Links>;

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
fn legacy_shape_and_skew_deserialize_as_segments() {
    let mut legacy = serde_json::to_value(LfoConfig::default()).unwrap();
    legacy.as_object_mut().unwrap().remove("segments");
    legacy["shape"] = serde_json::json!("Sine");
    legacy["skew"] = serde_json::json!({ "channels": [0.2, 0.8] });
    legacy["bipolar"] = serde_json::json!(true);
    legacy["smooth_time"] = serde_json::json!({ "channels": [0.01, 0.02] });

    let back: LfoConfig = serde_json::from_value(legacy).unwrap();

    assert!(back.segments.points().iter().all(|point| point.smooth));
    assert_eq!(back.segments.num_points(), 3);
    assert_eq!(back.segments.point(1).x, 0.2);
    assert_eq!(back.segments.point(1).y, 1.0);

    let mut with_segments = serde_json::to_value(LfoConfig {
        id: 4,
        segments: Segments::square(),
        ..LfoConfig::default()
    })
    .unwrap();
    with_segments["shape"] = serde_json::json!("Triangle");
    with_segments["skew"] = serde_json::json!(0.25);

    let back: LfoConfig = serde_json::from_value(with_segments.clone()).unwrap();

    assert_eq!(back.segments.points(), Segments::square().points());

    let saved = with_segments
        .as_object_mut()
        .unwrap()
        .remove("segments")
        .unwrap();
    with_segments["line"] = saved;

    let back: LfoConfig = serde_json::from_value(with_segments).unwrap();

    assert_eq!(back.segments.points(), Segments::square().points());
}

#[test]
fn table_knots_match_the_segments() {
    let lfo = make_lfo(Segments::sine());

    for index in [0, 1, 100, 1024, 2047] {
        let t = index as Smp / WAVEFORM_SIZE as Smp;
        let baked = lfo.wave[WAVEFORM_PAD_LEFT + index];

        assert!((baked - lfo.get_config().segments.value_at(t)).abs() < 1e-6);
        assert_eq!(
            TestLfo::read_wave(&lfo.wave, phase_at(index, 0.0)),
            baked,
            "index {index}"
        );
    }
}

#[test]
fn sine_midpoints_follow_catmull_rom() {
    let lfo = make_lfo(Segments::sine());

    for index in [0, 10, 500, 2046] {
        let t = (index as Smp + 0.5) / WAVEFORM_SIZE as Smp;
        let got = TestLfo::read_wave(&lfo.wave, phase_at(index, 0.5));
        let expected = lfo.get_config().segments.value_at(t);

        assert!(
            (got - expected).abs() < 1e-4,
            "index {index}: {got} vs {expected}"
        );
    }
}

#[test]
fn playback_applies_phase_shift() {
    let lfo = make_lfo(Segments::sine());
    let index = 0;
    let shift = 0.125;
    let read = TestLfo::read_wave(&lfo.wave, phase_at(index, 0.0).add_normalized(shift));
    let expected = lfo.get_config().segments.value_at(shift);

    assert!((read - expected).abs() < 1e-5, "{read} vs {expected}");
}

#[test]
fn set_segments_rebakes_the_table() {
    let mut lfo = make_lfo(Segments::triangle());
    let before = lfo.wave[WAVEFORM_PAD_LEFT + 100];

    lfo.set_segments(Segments::square());

    assert_ne!(lfo.wave[WAVEFORM_PAD_LEFT + 100], before);
    assert_eq!(
        lfo.get_config().segments.points(),
        Segments::square().points()
    );
    assert_eq!(lfo.wave[WAVEFORM_PAD_LEFT + 100], 1.0);
    assert_eq!(lfo.wave[WAVEFORM_PAD_LEFT + 1500], 0.0);

    lfo.set_segments(Segments::square());

    assert_eq!(
        lfo.get_config().segments.points(),
        Segments::square().points()
    );
}

#[test]
fn wrapped_read_stays_finite() {
    let lfo = make_lfo(Segments::square());
    let y = TestLfo::read_wave(&lfo.wave, Phase::from_bits(u32::MAX));

    assert!(y.is_finite());
}
