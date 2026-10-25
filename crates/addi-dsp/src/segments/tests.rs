use super::{CurvePoint, MAX_POINTS, Segment, Segments};
use crate::Smp;

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

fn triangle_at(skew: Smp) -> Segments {
    let skew = skew.clamp(0.0, 1.0);

    if skew <= 0.0 {
        return Segments::from_points(&[(0.0, 1.0), (1.0, 0.0)], false);
    }

    if skew >= 1.0 {
        return Segments::from_points(&[(0.0, 0.0), (1.0, 1.0)], false);
    }

    Segments::from_points(&[(0.0, 0.0), (skew, 1.0), (1.0, 0.0)], false)
}

fn square_at(skew: Smp) -> Segments {
    let skew = skew.clamp(0.0, 1.0);

    if skew <= 0.0 {
        return Segments::from_points(&[(0.0, 0.0), (1.0, 0.0)], false);
    }

    if skew >= 1.0 {
        return Segments::from_points(&[(0.0, 1.0), (1.0, 1.0)], false);
    }

    Segments::from_points(&[(0.0, 1.0), (skew, 1.0), (skew, 0.0), (1.0, 0.0)], false)
}

fn sine_at(skew: Smp) -> Segments {
    let mut segments = triangle_at(skew);

    for index in 0..segments.num_segments() {
        segments.set_smooth(index, true);
    }

    segments
}

fn render_open(segments: &Segments, len: usize) -> Vec<Smp> {
    let mut samples = vec![0.0; len];
    segments.render(&mut samples);
    samples
}

fn assert_points_eq(actual: &[Segment], expected: &[Segment]) {
    assert_eq!(actual.len(), expected.len());
    for (a, e) in actual.iter().zip(expected) {
        assert_eq!(a.x, e.x);
        assert_eq!(a.y, e.y);
        assert_eq!(a.power, e.power);
        assert_eq!(a.smooth, e.smooth);
    }
}

#[test]
fn serde_roundtrip() {
    let mut segments = Segments::default();
    segments.set_power(0, 4.0);
    segments.set_smooth(0, true);

    let expected = segments.segments().to_vec();
    let json = serde_json::to_value(&segments).unwrap();

    assert!(json.get("revision").is_none());

    let back: Segments = serde_json::from_value(json.clone()).unwrap();

    assert_points_eq(back.segments(), &expected);

    let again: Segments = serde_json::from_value(json).unwrap();

    assert_ne!(back.revision(), segments.revision());
    assert_ne!(back.revision(), again.revision());
}

#[test]
fn edits_advance_the_revision() {
    let mut segments = Segments::from_points(&[(0.0, 0.0), (1.0, 1.0)], false);
    let other = Segments::from_points(&[(0.0, 0.0), (1.0, 1.0)], false);

    assert_ne!(segments.revision(), other.revision());
    assert_eq!(segments, other);

    let revision = segments.revision();
    let cloned = segments.clone();

    assert_eq!(cloned.revision(), revision);
    assert!(!segments.remove_segment(0));
    assert_eq!(segments.revision(), revision);

    segments.set_power(0, 0.0);
    assert_ne!(segments.revision(), revision);
    assert_eq!(cloned.revision(), revision);
}

#[test]
fn smooth_applies_only_to_the_segment_that_starts_at_that_point() {
    let mut segments = Segments::default();
    let plain = Segments::default();

    segments.set_smooth(0, true);

    let smooth = render_open(&segments, 8);
    let plain = render_open(&plain, 8);

    assert!((smooth[1] - plain[1]).abs() > 1e-3);
    assert!((smooth[6] - plain[6]).abs() < 1e-5);
}

#[test]
fn legacy_shapes_match_the_old_curves() {
    let skew = 0.3;
    let triangle = render_open(&triangle_at(skew), 2048);
    let sine = render_open(&sine_at(skew), 2048);
    let square = render_open(&square_at(0.15), 2048);

    for index in 0..2048 {
        let t = index as Smp / 2048.0;

        assert!(
            (triangle[index] - legacy_triangle(t, skew)).abs() < 1e-5,
            "triangle {t}"
        );
        assert!(
            (sine[index] - legacy_sine(t, skew)).abs() < 1e-5,
            "sine {t}"
        );
        assert!(
            (square[index] - legacy_square(t, 0.15)).abs() < 1e-5,
            "square {t}"
        );
    }
}

#[test]
fn render_covers_the_half_open_period() {
    let ramp = Segments::from_points(&[(0.0, 0.0), (1.0, 1.0)], false);
    let mut samples = [0.0; 4];

    ramp.render(&mut samples);

    assert_eq!(samples, [0.0, 0.25, 0.5, 0.75]);

    let square = Segments::from_points(&[(0.0, 1.0), (0.5, 1.0), (0.5, 0.0), (1.0, 0.0)], false);
    let mut square_samples = [0.0; 8];

    square.render(&mut square_samples);

    assert_eq!(square_samples, [1.0, 1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0]);
}

#[test]
fn render_tails_match_an_aligned_render() {
    let mut segments = Segments::from_points(&[(0.0, 0.15), (0.37, 0.9), (1.0, 0.2)], true);
    segments.set_power(0, 4.0);
    segments.set_power(1, -2.5);

    let mut worst = 0.0f32;

    for &len in &[2, 3, 7, 8] {
        let mut short = vec![0.0; len];
        segments.render(&mut short);

        let long_len = len * 4;
        let mut long = vec![0.0; long_len];
        segments.render(&mut long);

        for (i, sample) in short.iter().enumerate() {
            let phase = i as Smp / len as Smp;
            let pos = phase * long_len as Smp;
            let index = pos.round() as usize;

            assert!((pos - index as Smp).abs() < 1e-3, "{phase}");
            worst = worst.max((sample - long[index]).abs());
        }
    }

    assert!(worst < 1e-5, "worst {worst}");
}

#[test]
fn value_at_reads_the_segment_that_contains_the_phase() {
    let ramp = Segments::from_points(&[(0.0, 0.0), (1.0, 1.0)], false);

    assert!((ramp.value_at(0.0) - 0.0).abs() < 1e-6);
    assert!((ramp.value_at(0.25) - 0.25).abs() < 1e-6);
    assert!((ramp.value_at(1.0) - 1.0).abs() < 1e-6);
    assert!((ramp.value_at(Smp::NAN) - ramp.value_at(0.0)).abs() < 1e-6);

    let square = Segments::from_points(
        &[(0.0, 0.0), (0.0, 1.0), (0.5, 1.0), (0.5, 0.0), (1.0, 0.0)],
        false,
    );

    assert!((square.value_at(0.0) - 1.0).abs() < 1e-6);
    assert!((square.value_at(0.25) - 1.0).abs() < 1e-6);
    assert!((square.value_at(0.5) - 1.0).abs() < 1e-6);
    assert!((square.value_at(0.75) - 0.0).abs() < 1e-6);

    let mut curved = Segments::from_points(&[(0.0, 0.0), (1.0, 1.0)], true);
    curved.set_power(0, 8.0);

    assert!(curved.value_at(0.5) < ramp.value_at(0.5));

    let mut samples = [0.0; 8];
    curved.render(&mut samples);
    assert!((curved.value_at(3.0 / 8.0) - samples[3]).abs() < 1e-5);
}

#[test]
fn render_points_closes_the_cycle_and_keeps_corners() {
    let ramp = Segments::from_points(&[(0.0, 0.0), (1.0, 1.0)], false);
    let mut points = Vec::new();
    let mut scratch = Vec::new();

    ramp.render_points(4, &mut points, &mut scratch);

    assert_eq!(scratch, vec![0.0, 0.25, 0.5, 0.75, 1.0]);
    assert!(points.windows(2).all(|pair| pair[1].x + 1e-5 >= pair[0].x));

    let triangle = Segments::from_points(&[(0.0, 0.0), (0.5, 1.0), (1.0, 0.0)], false);

    triangle.render_points(3, &mut points, &mut scratch);

    assert_eq!(
        points.iter().find(|point| (point.x - 0.5).abs() < 1e-6),
        Some(&CurvePoint { x: 0.5, y: 1.0 })
    );
    assert!(
        points
            .iter()
            .find(|point| (point.x - 1.0 / 3.0).abs() < 1e-5)
            .unwrap()
            .y
            < 1.0
    );

    let square = Segments::from_points(&[(0.0, 1.0), (0.5, 1.0), (0.5, 0.0), (1.0, 0.0)], false);

    square.render_points(4, &mut points, &mut scratch);

    assert_eq!(scratch, vec![1.0, 1.0, 1.0, 0.0, 0.0]);

    let at_half: Vec<Smp> = points
        .iter()
        .filter(|point| (point.x - 0.5).abs() < 1e-5)
        .map(|point| point.y)
        .collect();

    assert!(at_half.contains(&1.0));
    assert_eq!(at_half.last().copied(), Some(0.0));
}

#[test]
fn sanitize_clamps_curve_floats() {
    let mut segments =
        Segments::from_points(&[(0.2, -1.0), (0.5, Smp::INFINITY), (1.4, 0.25)], false);

    segments.sanitize();

    assert_eq!(segments.num_segments(), 3);
    assert_eq!(segments.segment(0).x, 0.0);
    assert_eq!(segments.segment(0).y, 0.0);
    assert_eq!(segments.segment(1).x, 0.5);
    assert_eq!(segments.segment(1).y, 0.0);
    assert_eq!(segments.segment(2).x, 1.0);
    assert_eq!(segments.segment(2).y, 0.25);

    let mut short: Segments = serde_json::from_str(r#"{"segments":[]}"#).unwrap();
    short.sanitize();

    assert_eq!(short.num_segments(), Segments::default().num_segments());
}

#[test]
fn span_matches_value_at_and_holds_the_end() {
    let mut segments =
        Segments::from_points(&[(0.0, 0.2), (0.3, 1.0), (0.3, 0.0), (1.0, 0.4)], false);

    segments.set_power(0, 3.0);
    segments.set_smooth(1, true);

    let steps = [0.0, 0.05, 0.17, 0.01, 0.4, 0.4, 0.0];
    let mut out = [0.0; 7];
    let mut phase = Smp::NAN;
    let end = segments.render_span(&mut out, phase, &steps);

    for (sample, step) in out.iter().zip(steps) {
        if !phase.is_finite() {
            phase = 0.0;
        }

        phase = phase.clamp(0.0, 1.0);

        assert!(
            (sample - segments.value_at(phase)).abs() < 1e-5,
            "phase {phase}: {sample} vs {}",
            segments.value_at(phase)
        );
        phase += step;
    }

    assert_eq!(end, phase.clamp(0.0, 1.0));
    assert_eq!(end, 1.0);

    let mut empty = [];
    assert_eq!(segments.render_span(&mut empty, 0.25, &[]), 0.25);
    assert_eq!(segments.render_span(&mut empty, 4.0, &[]), 1.0);
}

#[test]
fn span_can_leave_a_different_start_level() {
    let segments = Segments::from_points(&[(0.0, 0.0), (0.5, 1.0), (1.0, 0.25)], false);
    let steps = [0.25; 4];
    let mut out = [0.0; 4];
    let end = segments.render_span_from(&mut out, 0.0, &steps, 0.5);

    assert!((out[0] - 0.5).abs() < 1e-5);
    assert!((out[1] - 0.75).abs() < 1e-5);
    assert!((out[2] - 1.0).abs() < 1e-5);
    assert!((out[3] - 0.625).abs() < 1e-5);
    assert_eq!(end, 1.0);

    let mut phase = 0.0;

    for (sample, step) in out.iter().zip(steps) {
        assert!((sample - segments.value_from(phase, 0.5)).abs() < 1e-5);
        phase += step;
    }

    let mut drawn = [0.0; 4];
    segments.render_span(&mut drawn, 0.0, &steps);
    segments.render_span_from(&mut out, 0.0, &steps, 0.0);
    assert_eq!(out, drawn);
    assert!((segments.value_from(0.0, Smp::NAN) - segments.value_at(0.0)).abs() < 1e-6);
}

#[test]
fn positive_power_holds_the_segment_near_its_start() {
    let mut segments = Segments::from_points(&[(0.0, 0.0), (1.0, 1.0)], false);

    let mut samples = [0.0; 2];

    segments.render(&mut samples);
    assert!((samples[1] - 0.5).abs() < 1e-6);

    segments.set_power(0, 8.0);
    segments.render(&mut samples);

    assert!(samples[1] < 0.5);
}

#[test]
fn edits_keep_endpoints_and_reject_a_full_curve() {
    let mut segments = Segments::default();

    assert!(segments.add_segment(
        2,
        Segment {
            x: 0.75,
            y: 0.25,
            ..Segment::default()
        }
    ));
    assert_eq!(segments.num_segments(), 4);
    assert_eq!(segments.segment(2).x, 0.75);
    assert!(!segments.remove_segment(0));
    assert!(segments.remove_segment(2));
    assert_eq!(segments.num_segments(), 3);

    segments.flip_vertical();

    assert_eq!(segments.segment(0).y, 1.0);
    assert_eq!(segments.segment(1).y, 0.0);
    assert_eq!(segments.segment(2).y, 1.0);

    let mut full = Segments::from_points(&[(0.0, 0.0), (1.0, 1.0)], false);

    for i in 0..MAX_POINTS - 2 {
        assert!(full.add_segment(
            1,
            Segment {
                x: (i + 1) as Smp / MAX_POINTS as Smp,
                y: 0.5,
                ..Segment::default()
            }
        ));
    }

    assert!(!full.add_segment(
        1,
        Segment {
            x: 0.5,
            y: 0.5,
            ..Segment::default()
        }
    ));
    assert_eq!(full.num_segments(), MAX_POINTS);
}
