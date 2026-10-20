use super::*;

type TestLfo = Lfo<stub::Links>;

fn make_lfo(line: LfoLine) -> TestLfo {
    Lfo::from_config(
        &LfoConfig {
            id: 1,
            line,
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

fn skew_arg(arg: Smp, skew: Smp) -> Smp {
    let arg_less = arg < skew;

    Smp::from(arg_less) * (0.5 * arg / skew.max(Smp::EPSILON))
        + Smp::from(!arg_less) * (0.5 + (arg - skew) * 0.5 / (1.0 - skew).max(Smp::EPSILON))
}

fn legacy_triangle(t: Smp, skew: Smp) -> Smp {
    let x = skew_arg(t, skew);
    2.0 * x.min(1.0 - x)
}

fn legacy_square(t: Smp, skew: Smp) -> Smp {
    Smp::from(skew_arg(t, skew) < 0.5)
}

fn legacy_sine(t: Smp, skew: Smp) -> Smp {
    let sine = (std::f32::consts::PI * skew_arg(t, skew)).sin();
    sine * sine
}

#[test]
fn legacy_shape_and_skew_deserialize_as_a_line() {
    let mut legacy = serde_json::to_value(LfoConfig::default()).unwrap();
    legacy.as_object_mut().unwrap().remove("line");
    legacy["shape"] = serde_json::json!("Sine");
    legacy["skew"] = serde_json::json!({ "channels": [0.2, 0.8] });
    legacy["bipolar"] = serde_json::json!(true);
    legacy["smooth_time"] = serde_json::json!({ "channels": [0.01, 0.02] });

    let back: LfoConfig = serde_json::from_value(legacy).unwrap();

    assert!(back.line.smooth());
    assert_eq!(back.line.num_points(), 3);
    assert_eq!(back.line.point(1).x, 0.2);
    assert_eq!(back.line.point(1).y, 1.0);

    let mut with_line = serde_json::to_value(LfoConfig {
        id: 4,
        line: LfoLine::square(),
        ..LfoConfig::default()
    })
    .unwrap();
    with_line["shape"] = serde_json::json!("Triangle");
    with_line["skew"] = serde_json::json!(0.25);

    let back: LfoConfig = serde_json::from_value(with_line).unwrap();

    assert_eq!(back.line, LfoLine::square());
}

#[test]
fn line_roundtrips_and_falls_back_when_empty() {
    let mut line = LfoLine::triangle();
    line.set_power(0, 4.0);
    line.set_smooth(true);

    let json = serde_json::to_value(line).unwrap();
    let back: LfoLine = serde_json::from_value(json).unwrap();

    assert_eq!(back, line);

    let empty = serde_json::json!({ "points": [], "powers": [], "smooth": true });
    let back: LfoLine = serde_json::from_value(empty).unwrap();

    assert_eq!(back, LfoLine::triangle());
}

#[test]
fn legacy_shapes_match_the_old_curves() {
    let skew = 0.3;
    let triangle = LfoLine::triangle_at(skew);
    let sine = LfoLine::sine_at(skew);
    let square = LfoLine::square_at(0.15);

    for index in 0..WAVEFORM_SIZE {
        let t = index as Smp / WAVEFORM_SIZE as Smp;

        assert!(
            (triangle.value_at(t) - legacy_triangle(t, skew)).abs() < 1e-5,
            "triangle {t}"
        );
        assert!(
            (sine.value_at(t) - legacy_sine(t, skew)).abs() < 1e-5,
            "sine {t}"
        );
        assert!(
            (square.value_at(t) - legacy_square(t, 0.15)).abs() < 1e-5,
            "square {t}"
        );
    }
}

#[test]
fn table_knots_match_the_line() {
    let lfo = make_lfo(LfoLine::sine());

    for index in [0, 1, 100, 1024, 2047] {
        let t = index as Smp / WAVEFORM_SIZE as Smp;
        let baked = lfo.wave[WAVEFORM_PAD_LEFT + index];

        assert!((baked - lfo.get_config().line.value_at(t)).abs() < 1e-6);
        assert_eq!(
            TestLfo::read_wave(&lfo.wave, phase_at(index, 0.0)),
            baked,
            "index {index}"
        );
    }
}

#[test]
fn sine_midpoints_follow_catmull_rom() {
    let lfo = make_lfo(LfoLine::sine());

    for index in [0, 10, 500, 2046] {
        let t = (index as Smp + 0.5) / WAVEFORM_SIZE as Smp;
        let got = TestLfo::read_wave(&lfo.wave, phase_at(index, 0.5));
        let expected = lfo.get_config().line.value_at(t);

        assert!(
            (got - expected).abs() < 1e-4,
            "index {index}: {got} vs {expected}"
        );
    }
}

#[test]
fn playback_applies_phase_shift() {
    let lfo = make_lfo(LfoLine::sine());
    let index = 0;
    let shift = 0.125;
    let read = TestLfo::read_wave(&lfo.wave, phase_at(index, 0.0).add_normalized(shift));
    let expected = lfo.get_config().line.value_at(shift);

    assert!((read - expected).abs() < 1e-5, "{read} vs {expected}");
}

#[test]
fn set_line_rebakes_the_table() {
    let mut lfo = make_lfo(LfoLine::triangle());
    let before = lfo.wave[WAVEFORM_PAD_LEFT + 100];

    lfo.set_line(LfoLine::square());

    assert_ne!(lfo.wave[WAVEFORM_PAD_LEFT + 100], before);
    assert_eq!(lfo.get_config().line, LfoLine::square());
    assert_eq!(lfo.wave[WAVEFORM_PAD_LEFT + 100], 1.0);
    assert_eq!(lfo.wave[WAVEFORM_PAD_LEFT + 1500], 0.0);

    lfo.set_line(LfoLine::square());

    assert_eq!(lfo.get_config().line, LfoLine::square());
}

#[test]
fn positive_power_holds_the_segment_near_its_start() {
    let mut line = LfoLine::linear();

    assert!((line.value_at(0.5) - 0.5).abs() < 1e-6);

    line.set_power(0, 8.0);

    assert!(line.value_at(0.5) < 0.5);
}

#[test]
fn edits_keep_endpoints_and_reject_a_full_curve() {
    let mut line = LfoLine::triangle();

    assert!(line.add_point(2, LfoPoint { x: 0.75, y: 0.25 }));
    assert_eq!(line.num_points(), 4);
    assert_eq!(line.point(2).x, 0.75);
    assert!(!line.remove_point(0));
    assert!(line.remove_point(2));
    assert_eq!(line.num_points(), 3);

    line.flip_vertical();

    assert_eq!(line.point(0).y, 1.0);
    assert_eq!(line.point(1).y, 0.0);
    assert_eq!(line.point(2).y, 1.0);

    let mut full = LfoLine::linear();

    for i in 0..MAX_POINTS - 2 {
        assert!(full.add_point(
            1,
            LfoPoint {
                x: (i + 1) as Smp / MAX_POINTS as Smp,
                y: 0.5,
            }
        ));
    }

    assert!(!full.add_point(1, LfoPoint { x: 0.5, y: 0.5 }));
    assert_eq!(full.num_points(), MAX_POINTS);
}

#[test]
fn wrapped_read_stays_finite() {
    let lfo = make_lfo(LfoLine::square());
    let y = TestLfo::read_wave(&lfo.wave, Phase::from_bits(u32::MAX));

    assert!(y.is_finite());
}
