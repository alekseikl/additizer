use super::{MAX_TIME, TimePoint, TimeSegments, view_span};

#[test]
fn default_is_a_one_second_fall() {
    let segments = TimeSegments::default();

    assert_eq!(segments.num_points(), 2);
    assert_eq!(segments.duration(0), 1.0);
    assert_eq!(segments.total_time(), 1.0);
    assert!((segments.value_at(0.0) - 1.0).abs() < 1e-5);
    assert!((segments.value_at(0.5) - 0.5).abs() < 1e-5);
    assert!((segments.value_at(1.0) - 0.0).abs() < 1e-5);
    assert!((segments.value_at(4.0) - 0.0).abs() < 1e-5);
}

#[test]
fn sanitize_orders_times_and_pins_the_start() {
    let segments = TimeSegments::from_points(&[(2.0, -1.0), (0.25, 2.0), (0.5, 0.4)], false);

    assert_eq!(segments.time(0), 0.0);
    assert_eq!(segments.point(0).y, 0.0);
    assert!(segments.time(1) >= segments.time(0));
    assert!(segments.time(2) >= segments.time(1));
    assert_eq!(segments.point(1).y, 1.0);
    assert!((segments.time(2) - 0.5).abs() < 1e-6);
    assert!((segments.duration(0) - 0.25).abs() < 1e-6);
    assert!((segments.point(2).duration).abs() < 1e-6);
}

#[test]
fn moving_a_point_changes_only_the_previous_duration() {
    let mut segments = TimeSegments::from_points(&[(0.0, 0.0), (0.4, 1.0), (1.0, 0.0)], false);
    let middle = segments.point(1);

    segments.set_time(1, 5.0);
    segments.set_point(1, TimePoint { y: 0.2, ..middle });

    assert!((segments.duration(0) - 5.0).abs() < 1e-6);
    assert!((segments.duration(1) - 0.6).abs() < 1e-6);
    assert!((segments.time(1) - 5.0).abs() < 1e-6);
    assert!((segments.time(2) - 5.6).abs() < 1e-6);
    assert!((segments.total_time() - 5.6).abs() < 1e-6);
    assert!((segments.point(1).y - 0.2).abs() < 1e-6);

    segments.set_time(2, 80.0);
    assert!((segments.total_time() - MAX_TIME).abs() < 1e-4);
    assert!((segments.duration(0) - 5.0).abs() < 1e-4);
    assert!((segments.time(2) - MAX_TIME).abs() < 1e-4);

    let start = segments.point(0);
    let duration = segments.duration(0);

    segments.set_time(0, 3.0);
    segments.set_point(0, TimePoint { y: 0.7, ..start });
    assert_eq!(segments.time(0), 0.0);
    assert!((segments.point(0).y - 0.7).abs() < 1e-6);
    assert!((segments.duration(0) - duration).abs() < 1e-6);
}

#[test]
fn add_and_remove_keep_the_ends() {
    let mut segments = TimeSegments::from_points(&[(0.0, 1.0), (1.0, 0.0)], false);

    assert!(!segments.add_point(0, 0.0, TimePoint::default()));
    assert!(segments.add_point(
        1,
        0.4,
        TimePoint {
            y: 0.5,
            ..TimePoint::default()
        },
    ));
    assert_eq!(segments.num_segments(), 2);
    assert!((segments.time(1) - 0.4).abs() < 1e-6);
    assert!((segments.duration(0) - 0.4).abs() < 1e-6);
    assert!((segments.duration(1) - 0.6).abs() < 1e-6);
    assert!((segments.total_time() - 1.0).abs() < 1e-6);
    assert!(!segments.remove_point(0));
    assert!(!segments.remove_point(2));
    assert!(segments.remove_point(1));
    assert_eq!(segments.num_points(), 2);
    assert!((segments.total_time() - 1.0).abs() < 1e-6);
    assert!((segments.duration(0) - 1.0).abs() < 1e-6);
}

#[test]
fn zero_length_segment_renders_nothing() {
    let start = TimePoint {
        y: 1.0,
        ..TimePoint::default()
    };
    let end = TimePoint {
        y: 0.0,
        ..TimePoint::default()
    };
    let mut out = [0.25; 4];

    assert_eq!(
        TimePoint::render_segment(&mut out, 0.0, 0.01, start, end, 0.0),
        Err(0)
    );
    assert_eq!(out, [0.25; 4]);
    assert!((TimePoint::level_between(start, end, 0.3) - 0.0).abs() < 1e-6);
}

#[test]
fn render_segment_uses_the_given_duration() {
    let start = TimePoint {
        duration: 1.0,
        y: 1.0,
        ..TimePoint::default()
    };
    let end = TimePoint {
        y: 0.0,
        ..TimePoint::default()
    };
    let mut out = [0.0; 4];

    assert_eq!(
        TimePoint::render_segment(&mut out, 0.0, 0.25, start, end, 1.0),
        Err(4)
    );

    for (index, sample) in out.iter().enumerate() {
        let expected = 1.0 - index as f32 * 0.25;

        assert!((sample - expected).abs() < 1e-5, "{sample} vs {expected}");
    }
}

#[test]
fn render_points_keeps_the_peak() {
    let segments = TimeSegments::from_points(&[(0.0, 0.0), (0.5, 1.0), (1.0, 0.0)], false);
    let mut points = Vec::new();
    let mut scratch = Vec::new();

    segments.render_points(
        8,
        view_span(segments.total_time()),
        &mut points,
        &mut scratch,
    );

    assert!(
        points
            .iter()
            .any(|point| (point[0] - 0.5).abs() < 1e-5 && (point[1] - 1.0).abs() < 1e-5)
    );
}

#[test]
fn render_points_keeps_time_past_the_view() {
    let segments = TimeSegments::from_points(&[(0.0, 1.0), (2.0, 0.0)], false);
    let mut points = Vec::new();
    let mut scratch = Vec::new();

    segments.render_points(4, 1.0, &mut points, &mut scratch);

    let edge = points
        .iter()
        .filter(|point| point[0] <= 1.0 + 1e-4)
        .max_by(|a, b| a[0].total_cmp(&b[0]))
        .copied()
        .unwrap();

    assert!((edge[0] - 1.0).abs() < 1e-4, "{edge:?}");
    assert!((edge[1] - 0.5).abs() < 1e-4, "{edge:?}");
}

#[test]
fn serde_roundtrip_keeps_markers() {
    let mut segments = TimeSegments::from_points(&[(0.0, 1.0), (0.3, 0.4), (0.8, 0.0)], true);
    assert!(segments.set_loop_point(Some(0)));
    assert!(segments.set_release_point(Some(1)));

    let json = serde_json::to_string(&segments).unwrap();
    let restored: TimeSegments = serde_json::from_str(&json).unwrap();

    assert_eq!(restored.points(), segments.points());
    assert_eq!(restored.loop_point_idx(), Some(0));
    assert_eq!(restored.release_point_idx(), Some(1));
    assert!(json.contains("duration"));
    assert!(!json.contains("\"time\""));
}

#[test]
fn flip_horizontal_reverses_durations() {
    let mut segments = TimeSegments::from_points(&[(0.0, 1.0), (0.25, 0.5), (1.0, 0.0)], false);

    segments.set_power(0, 2.0);
    segments.flip_horizontal();

    assert!(segments.time(0).abs() < 1e-6);
    assert!((segments.time(1) - 0.75).abs() < 1e-5);
    assert!((segments.time(2) - 1.0).abs() < 1e-5);
    assert!(segments.point(0).y.abs() < 1e-6);
    assert!((segments.point(1).y - 0.5).abs() < 1e-6);
    assert!((segments.point(2).y - 1.0).abs() < 1e-6);
    assert!((segments.duration(0) - 0.75).abs() < 1e-5);
    assert!((segments.duration(1) - 0.25).abs() < 1e-5);
    assert!((segments.power(1) + 2.0).abs() < 1e-6);
}

#[test]
fn short_json_becomes_the_default() {
    let mut segments: TimeSegments = serde_json::from_str(r#"{"points":[]}"#).unwrap();

    segments.sanitize();

    assert_eq!(segments.num_points(), TimeSegments::default().num_points());
    assert!((segments.total_time() - 1.0).abs() < 1e-6);
}

#[test]
fn view_leaves_room_past_the_curve() {
    assert!((view_span(0.0) - 1.0).abs() < 1e-6);
    assert!((view_span(1.0) - 1.25).abs() < 1e-5);
    assert!((view_span(MAX_TIME) - MAX_TIME).abs() < 1e-4);
}
