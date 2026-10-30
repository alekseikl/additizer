use super::{MAX_POINTS, SegPoint, Segments};
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

    for index in 0..segments.num_points() {
        segments.set_smooth(index, true);
    }

    segments
}

fn render_open(segments: &Segments, len: usize) -> Vec<Smp> {
    let mut samples = vec![0.0; len];
    segments.render_waveform(&mut samples);
    samples
}

fn assert_points_eq(actual: &[SegPoint], expected: &[SegPoint]) {
    assert_eq!(actual.len(), expected.len());
    for (a, e) in actual.iter().zip(expected) {
        assert_eq!(a.x, e.x);
        assert_eq!(a.y, e.y);
        assert_eq!(a.power, e.power);
        assert_eq!(a.smooth, e.smooth);
    }
}

#[test]
fn unity_is_two_unsmoothed_endpoints() {
    let unity = Segments::from_points(&[(0.0, 0.0), (1.0, 1.0)], false);
    assert!(unity.is_unity());

    assert!(!Segments::from_points(&[(0.0, 0.0), (1.0, 1.0)], true).is_unity());
    assert!(!Segments::from_points(&[(0.0, 0.0), (0.4, 0.4), (1.0, 1.0)], false).is_unity());
    assert!(!Segments::from_points(&[(0.0, 0.0), (1.0, 0.5)], false).is_unity());

    let mut bent = Segments::from_points(&[(0.0, 0.0), (1.0, 1.0)], false);
    bent.set_power(0, 2.0);
    assert!(!bent.is_unity());

    let mut end = Segments::from_points(&[(0.0, 0.0), (1.0, 1.0)], false);
    end.set_power(1, 1.0);
    end.set_smooth(1, true);
    assert!(end.is_unity());
}

#[test]
fn serde_roundtrip() {
    let mut segments = Segments::default();
    segments.set_power(0, 4.0);
    segments.set_smooth(0, true);

    let expected = segments.points().to_vec();
    let json = serde_json::to_value(&segments).unwrap();

    assert!(json.get("revision").is_none());

    let back: Segments = serde_json::from_value(json.clone()).unwrap();

    assert_points_eq(back.points(), &expected);

    let again: Segments = serde_json::from_value(json).unwrap();

    assert_ne!(back.revision(), segments.revision());
    assert_ne!(back.revision(), again.revision());
}

#[test]
fn edits_advance_the_revision() {
    let mut segments = Segments::from_points(&[(0.0, 0.0), (1.0, 1.0)], false);
    let other = Segments::from_points(&[(0.0, 0.0), (1.0, 1.0)], false);

    assert_ne!(segments.revision(), other.revision());
    assert_points_eq(segments.points(), other.points());

    let revision = segments.revision();
    let cloned = segments.clone();

    assert_eq!(cloned.revision(), revision);
    assert!(!segments.remove_point(0));
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
fn render_closed_includes_both_endpoints() {
    let ramp = Segments::from_points(&[(0.0, 0.0), (1.0, 1.0)], false);
    let mut samples = [0.0; 5];

    ramp.render_closed(&mut samples);

    assert_eq!(samples, [0.0, 0.25, 0.5, 0.75, 1.0]);

    let end = Segments::from_points(&[(0.0, 0.2), (1.0, 0.8)], false);
    let mut pair = [0.0; 2];

    end.render_closed(&mut pair);

    assert_eq!(pair, [0.2, 0.8]);
}

#[test]
fn render_covers_the_half_open_period() {
    let ramp = Segments::from_points(&[(0.0, 0.0), (1.0, 1.0)], false);
    let mut samples = [0.0; 4];

    ramp.render_waveform(&mut samples);

    assert_eq!(samples, [0.0, 0.25, 0.5, 0.75]);

    let square = Segments::from_points(&[(0.0, 1.0), (0.5, 1.0), (0.5, 0.0), (1.0, 0.0)], false);
    let mut square_samples = [0.0; 8];

    square.render_waveform(&mut square_samples);

    assert_eq!(square_samples, [1.0, 1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0]);

    let leading = Segments::from_points(
        &[(0.0, 0.0), (0.0, 1.0), (0.5, 1.0), (0.5, 0.0), (1.0, 0.0)],
        false,
    );
    let mut leading_samples = [0.0; 8];

    leading.render_waveform(&mut leading_samples);

    assert_eq!(leading_samples, [1.0, 1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0]);
}

#[test]
fn render_tails_match_an_aligned_render() {
    let mut segments = Segments::from_points(&[(0.0, 0.15), (0.37, 0.9), (1.0, 0.2)], true);
    segments.set_power(0, 4.0);
    segments.set_power(1, -2.5);

    let mut worst = 0.0f32;

    for &len in &[2, 3, 7, 8] {
        let mut short = vec![0.0; len];
        segments.render_waveform(&mut short);

        let long_len = len * 4;
        let mut long = vec![0.0; long_len];
        segments.render_waveform(&mut long);

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
fn phase_at_walks_the_segment_that_starts_at_that_point() {
    let segments = Segments::from_points(&[(0.0, 0.0), (0.25, 1.0), (1.0, 0.0)], false);

    assert!((segments.phase_at(0, 0.0) - 0.0).abs() < 1e-6);
    assert!((segments.phase_at(0, 0.5) - 0.125).abs() < 1e-6);
    assert!((segments.phase_at(0, 2.0) - 0.25).abs() < 1e-6);
    assert!((segments.phase_at(1, 0.0) - 0.25).abs() < 1e-6);
    assert!((segments.phase_at(1, 1.0) - 1.0).abs() < 1e-6);
    assert!((segments.phase_at(2, 0.0) - 1.0).abs() < 1e-6);
}

#[test]
fn value_at_reads_the_segment_that_contains_the_phase() {
    let ramp = Segments::from_points(&[(0.0, 0.0), (1.0, 1.0)], false);

    assert!((ramp.value_at(0.0, None, None) - 0.0).abs() < 1e-6);
    assert!((ramp.value_at(0.25, None, None) - 0.25).abs() < 1e-6);
    assert!((ramp.value_at(1.0, None, None) - 1.0).abs() < 1e-6);
    assert!((ramp.value_at(0.0, Some(0.4), None) - 0.4).abs() < 1e-6);

    let square = Segments::from_points(
        &[(0.0, 0.0), (0.0, 1.0), (0.5, 1.0), (0.5, 0.0), (1.0, 0.0)],
        false,
    );

    assert!((square.value_at(0.0, None, None) - 1.0).abs() < 1e-6);
    assert!((square.value_at(0.25, None, None) - 1.0).abs() < 1e-6);
    assert!((square.value_at(0.5, None, None) - 1.0).abs() < 1e-6);
    assert!((square.value_at(0.75, None, None) - 0.0).abs() < 1e-6);

    let mut curved = Segments::from_points(&[(0.0, 0.0), (1.0, 1.0)], true);
    curved.set_power(0, 8.0);

    assert!(curved.value_at(0.5, None, None) < ramp.value_at(0.5, None, None));

    let mut samples = [0.0; 8];
    curved.render_waveform(&mut samples);
    assert!((curved.value_at(3.0 / 8.0, None, None) - samples[3]).abs() < 1e-5);
}

#[test]
fn render_points_closes_the_cycle_and_keeps_corners() {
    let ramp = Segments::from_points(&[(0.0, 0.0), (1.0, 1.0)], false);
    let mut points = Vec::new();
    let mut scratch = Vec::new();

    ramp.render_points(4, &mut points, &mut scratch);

    assert_eq!(scratch, vec![0.0, 0.25, 0.5, 0.75, 1.0]);
    assert!(
        points
            .windows(2)
            .all(|pair| pair[1][0] + 1e-5 >= pair[0][0])
    );

    let triangle = Segments::from_points(&[(0.0, 0.0), (0.5, 1.0), (1.0, 0.0)], false);

    triangle.render_points(3, &mut points, &mut scratch);

    assert_eq!(
        points.iter().find(|point| (point[0] - 0.5).abs() < 1e-6),
        Some(&[0.5, 1.0])
    );
    assert!(
        points
            .iter()
            .find(|point| (point[0] - 1.0 / 3.0).abs() < 1e-5)
            .unwrap()[1]
            < 1.0
    );

    let square = Segments::from_points(&[(0.0, 1.0), (0.5, 1.0), (0.5, 0.0), (1.0, 0.0)], false);

    square.render_points(4, &mut points, &mut scratch);

    assert_eq!(scratch, vec![1.0, 1.0, 1.0, 0.0, 0.0]);

    let at_half: Vec<Smp> = points
        .iter()
        .filter(|point| (point[0] - 0.5).abs() < 1e-5)
        .map(|point| point[1])
        .collect();

    assert!(at_half.contains(&1.0));
    assert_eq!(at_half.last().copied(), Some(0.0));
}

#[test]
fn render_points_keeps_a_vertical_step() {
    let saw = Segments::from_points(&[(0.0, 0.0), (1.0, 1.0), (1.0, 0.0)], false);
    let mut points = Vec::new();
    let mut scratch = Vec::new();

    saw.render_points(4, &mut points, &mut scratch);

    let steps: Vec<_> = points
        .windows(2)
        .filter(|pair| pair[0][0] == pair[1][0])
        .collect();

    assert_eq!(steps, vec![&[[1.0, 1.0], [1.0, 0.0]]]);
    assert!(points.windows(2).all(|pair| pair[0] != pair[1]));

    let square = Segments::from_points(
        &[(0.0, 0.0), (0.0, 1.0), (0.5, 1.0), (0.5, 0.0), (1.0, 0.0)],
        false,
    );

    square.render_points(4, &mut points, &mut scratch);

    let at = |x: Smp| -> Vec<Smp> {
        points
            .iter()
            .filter(|point| point[0] == x)
            .map(|point| point[1])
            .collect()
    };

    assert_eq!(at(0.0), vec![0.0, 1.0]);
    assert_eq!(at(0.5), vec![1.0, 0.0]);
    assert!(points.windows(2).all(|pair| pair[0] != pair[1]));
}

#[test]
fn sanitize_clamps_curve_floats() {
    let mut segments =
        Segments::from_points(&[(0.2, -1.0), (0.5, Smp::INFINITY), (1.4, 0.25)], false);

    segments.sanitize();

    assert_eq!(segments.num_points(), 3);
    assert_eq!(segments.point(0).x, 0.0);
    assert_eq!(segments.point(0).y, 0.0);
    assert_eq!(segments.point(1).x, 0.5);
    assert_eq!(segments.point(1).y, 0.0);
    assert_eq!(segments.point(2).x, 1.0);
    assert_eq!(segments.point(2).y, 0.25);

    let short: Segments = serde_json::from_str(r#"{"segments":[]}"#).unwrap();
    let one: Segments =
        serde_json::from_str(r#"{"segments":[{"x":0.2,"y":0.3,"power":0.0,"smooth":false}]}"#)
            .unwrap();

    assert_eq!(short.num_points(), Segments::default().num_points());
    assert_eq!(one.num_points(), Segments::default().num_points());
}

#[test]
#[should_panic(expected = "segments must have 2 to 100 points")]
fn from_points_rejects_a_single_point() {
    let _ = Segments::from_points(&[(0.0, 0.0)], false);
}

#[test]
fn positive_power_holds_the_segment_near_its_start() {
    let mut segments = Segments::from_points(&[(0.0, 0.0), (1.0, 1.0)], false);

    let mut samples = [0.0; 2];

    segments.render_waveform(&mut samples);
    assert!((samples[1] - 0.5).abs() < 1e-6);

    segments.set_power(0, 8.0);
    segments.render_waveform(&mut samples);

    assert!(samples[1] < 0.5);
}

#[test]
fn edits_keep_endpoints_and_reject_a_full_curve() {
    let mut segments = Segments::default();

    assert!(segments.add_point(
        2,
        SegPoint {
            x: 0.75,
            y: 0.25,
            ..SegPoint::default()
        }
    ));
    assert_eq!(segments.num_points(), 4);
    assert_eq!(segments.point(2).x, 0.75);
    assert!(!segments.remove_point(0));
    assert!(segments.remove_point(2));
    assert_eq!(segments.num_points(), 3);

    segments.flip_vertical();

    assert_eq!(segments.point(0).y, 1.0);
    assert_eq!(segments.point(1).y, 0.0);
    assert_eq!(segments.point(2).y, 1.0);

    let mut full = Segments::from_points(&[(0.0, 0.0), (1.0, 1.0)], false);

    for i in 0..MAX_POINTS - 2 {
        assert!(full.add_point(
            1,
            SegPoint {
                x: (i + 1) as Smp / MAX_POINTS as Smp,
                y: 0.5,
                ..SegPoint::default()
            }
        ));
    }

    assert!(!full.add_point(
        1,
        SegPoint {
            x: 0.5,
            y: 0.5,
            ..SegPoint::default()
        }
    ));
    assert_eq!(full.num_points(), MAX_POINTS);
}

#[test]
fn loop_and_release_follow_the_point_rules() {
    let mut segments = Segments::from_points(&[(0.0, 0.0), (0.5, 1.0), (1.0, 0.0)], false);

    assert!(!segments.set_loop_point(Some(2)));
    assert!(segments.set_loop_point(Some(0)));
    assert!(segments.set_release_point(Some(1)));
    assert_eq!(segments.loop_point_idx(), Some(0));
    assert!(!segments.set_loop_point(Some(1)));
    assert!(!segments.set_loop_point(Some(2)));
    assert!(!segments.set_loop_point(Some(0)));

    assert!(segments.set_release_point(Some(0)));
    assert_eq!(segments.loop_point_idx(), None);
    assert_eq!(segments.release_point_idx(), Some(0));
    assert!(!segments.set_loop_point(Some(0)));

    assert!(segments.set_release_point(None));
    assert!(segments.set_loop_point(Some(0)));
    assert!(segments.set_release_point(Some(2)));
    assert!(segments.set_release_point(None));
    assert_eq!(segments.loop_point_idx(), Some(0));
    assert_eq!(segments.release_point_idx(), None);
}

#[test]
fn markers_follow_inserted_and_removed_points() {
    let mut segments =
        Segments::from_points(&[(0.0, 0.0), (0.3, 0.5), (0.7, 1.0), (1.0, 0.0)], false);
    assert!(segments.set_loop_point(Some(1)));
    assert!(segments.set_release_point(Some(2)));

    assert!(segments.add_point(
        1,
        SegPoint {
            x: 0.15,
            y: 0.2,
            ..SegPoint::default()
        }
    ));
    assert_eq!(segments.loop_point_idx(), Some(2));
    assert_eq!(segments.release_point_idx(), Some(3));

    assert!(segments.remove_point(2));
    assert_eq!(segments.loop_point_idx(), None);
    assert_eq!(segments.release_point_idx(), Some(2));
}

#[test]
fn horizontal_flip_mirrors_markers_and_drops_an_illegal_loop() {
    let mut segments =
        Segments::from_points(&[(0.0, 0.0), (0.25, 0.5), (0.75, 1.0), (1.0, 0.0)], false);
    assert!(segments.set_loop_point(Some(1)));
    assert!(segments.set_release_point(Some(2)));

    segments.flip_horizontal();

    assert_eq!(segments.loop_point_idx(), None);
    assert_eq!(segments.release_point_idx(), Some(1));
}

#[test]
fn markers_round_trip_and_missing_ones_stay_clear() {
    let mut segments = Segments::from_points(&[(0.0, 0.0), (0.5, 1.0), (1.0, 0.0)], false);
    assert!(segments.set_loop_point(Some(0)));
    assert!(segments.set_release_point(Some(1)));

    let json = serde_json::to_value(&segments).unwrap();
    assert_eq!(json["loop_point"], 0);
    assert_eq!(json["release_point"], 1);
    assert!(json.get("revision").is_none());

    let back: Segments = serde_json::from_value(json).unwrap();
    assert_eq!(back.loop_point_idx(), Some(0));
    assert_eq!(back.release_point_idx(), Some(1));
    assert_points_eq(back.points(), segments.points());

    let bare: Segments = serde_json::from_str(
        r#"{"segments":[{"x":0.0,"y":0.0,"power":0.0,"smooth":false},{"x":1.0,"y":1.0,"power":0.0,"smooth":false}]}"#,
    )
    .unwrap();
    assert_eq!(bare.loop_point_idx(), None);
    assert_eq!(bare.release_point_idx(), None);

    let illegal: Segments = serde_json::from_str(
        r#"{"segments":[{"x":0.0,"y":0.0,"power":0.0,"smooth":false},{"x":1.0,"y":1.0,"power":0.0,"smooth":false}],"loop_point":1}"#,
    )
    .unwrap();
    assert_eq!(illegal.loop_point_idx(), None);
}

#[test]
fn loop_samples_span_the_loop_at_the_phase_step() {
    let mut segments =
        Segments::from_points(&[(0.0, 0.0), (0.25, 1.0), (0.75, 0.5), (1.0, 0.0)], false);

    assert_eq!(segments.loop_samples(0.25), 0);

    assert!(segments.set_loop_point(Some(1)));
    assert_eq!(segments.loop_samples(0.25), 3);
    assert_eq!(segments.loop_samples(0.0), 0);
    assert_eq!(segments.loop_samples(Smp::EPSILON / 2.0), 0);

    assert!(segments.set_release_point(Some(2)));
    assert_eq!(segments.loop_samples(0.25), 2);

    let mut flat = Segments::from_points(&[(0.0, 0.2), (1.0, 0.5), (1.0, 0.8)], false);
    assert!(flat.set_loop_point(Some(1)));
    assert_eq!(flat.loop_samples(0.25), 0);
    assert_eq!(flat.loop_samples(0.0), 0);
}

#[test]
fn release_level_replaces_the_segment_that_leaves_that_point() {
    let mut segments = Segments::from_points(&[(0.0, 0.0), (0.5, 1.0), (1.0, 0.0)], false);
    assert!(segments.set_release_point(Some(1)));

    assert!((segments.value_at(0.5, None, Some(0.25)) - 0.25).abs() < 1e-6);
    assert!((segments.value_at(0.75, None, Some(0.25)) - 0.125).abs() < 1e-5);
    assert!((segments.value_at(0.25, None, Some(0.25)) - 0.5).abs() < 1e-5);
    assert!((segments.value_at(0.5, None, None) - 1.0).abs() < 1e-6);

    assert!(segments.set_release_point(Some(2)));
    assert!((segments.value_at(1.0, None, Some(0.4)) - 0.4).abs() < 1e-6);
    assert!((segments.value_at(0.75, None, Some(0.4)) - 0.5).abs() < 1e-5);
}
