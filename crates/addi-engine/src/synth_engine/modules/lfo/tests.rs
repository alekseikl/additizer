use super::*;

type TestLfo = Lfo<stub::Links>;

fn make_lfo(shape: LfoShape, skew: Smp) -> TestLfo {
    Lfo::from_config(
        &LfoConfig {
            id: 1,
            shape,
            skew,
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

fn analytic(shape: LfoShape, t: Smp, skew: Smp) -> Smp {
    TestLfo::shape_function(shape)(TestLfo::skew_arg(t, skew))
}

#[test]
fn skew_deserializes_mono_and_legacy_stereo() {
    let config = LfoConfig {
        id: 3,
        skew: 0.25,
        ..LfoConfig::default()
    };
    let json = serde_json::to_value(&config).unwrap();

    assert_eq!(json["skew"], 0.25);

    let back: LfoConfig = serde_json::from_value(json).unwrap();

    assert_eq!(back.skew, 0.25);

    let mut legacy = serde_json::to_value(LfoConfig::default()).unwrap();
    legacy["skew"] = serde_json::json!({ "channels": [0.2, 0.8] });
    legacy["bipolar"] = serde_json::json!(true);
    legacy["smooth_time"] = serde_json::json!({ "channels": [0.01, 0.02] });

    let back: LfoConfig = serde_json::from_value(legacy).unwrap();

    assert_eq!(back.skew, 0.2);
}

#[test]
fn table_knots_match_shape_and_skew() {
    let skew = 0.3;
    let lfo = make_lfo(LfoShape::Triangle, skew);

    for index in [0, 1, 100, 1024, 2047] {
        let t = index as Smp / WAVEFORM_SIZE as Smp;
        let baked = lfo.wave[WAVEFORM_PAD_LEFT + index];

        assert_eq!(baked, analytic(LfoShape::Triangle, t, skew));
        assert_eq!(
            TestLfo::read_wave(&lfo.wave, phase_at(index, 0.0)),
            baked,
            "index {index}"
        );
    }
}

#[test]
fn sine_midpoints_follow_catmull_rom() {
    let skew = 0.5;
    let lfo = make_lfo(LfoShape::Sine, skew);

    for index in [0, 10, 500, 2046] {
        let t = (index as Smp + 0.5) / WAVEFORM_SIZE as Smp;
        let got = TestLfo::read_wave(&lfo.wave, phase_at(index, 0.5));
        let expected = analytic(LfoShape::Sine, t, skew);

        assert!(
            (got - expected).abs() < 1e-5,
            "index {index}: {got} vs {expected}"
        );
    }
}

#[test]
fn playback_applies_phase_shift() {
    let skew = 0.5;
    let lfo = make_lfo(LfoShape::Sine, skew);
    let index = 100;
    let t = index as Smp / WAVEFORM_SIZE as Smp;
    let shift = 0.125;
    let read = TestLfo::read_wave(&lfo.wave, phase_at(index, 0.0).add_normalized(shift));
    let expected = TestLfo::evaluate(LfoShape::Sine, t, shift, skew);

    assert!((read - expected).abs() < 1e-5, "{read} vs {expected}");
}

#[test]
fn shape_and_skew_rebake_the_table() {
    let mut lfo = make_lfo(LfoShape::Triangle, 0.5);
    let before = lfo.wave[WAVEFORM_PAD_LEFT + 100];

    lfo.set_skew(0.2);

    assert_ne!(lfo.wave[WAVEFORM_PAD_LEFT + 100], before);
    assert_eq!(lfo.get_config().skew, 0.2);

    lfo.set_skew(2.0);

    assert_eq!(lfo.get_config().skew, 1.0);
    assert_eq!(make_lfo(LfoShape::Sine, -0.4).get_config().skew, 0.0);

    lfo.set_skew(0.5);
    lfo.set_shape(LfoShape::Square);

    let high = lfo.wave[WAVEFORM_PAD_LEFT + 100];
    let low = lfo.wave[WAVEFORM_PAD_LEFT + 1500];

    assert_eq!(high, 1.0);
    assert_eq!(low, 0.0);
}

#[test]
fn wrapped_read_stays_finite() {
    let lfo = make_lfo(LfoShape::Square, 0.15);
    let y = TestLfo::read_wave(&lfo.wave, Phase::from_bits(u32::MAX));

    assert!(y.is_finite());
}
