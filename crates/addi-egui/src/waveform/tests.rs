use egui::Vec2;

use super::*;

#[test]
fn scales_down_overflowing_peaks() {
    let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(4.0, 100.0));
    let center_y = rect.center().y;
    let mut points = vec![
        Pos2::new(0.0, center_y - 40.0),
        Pos2::new(1.0, center_y + 80.0),
        Pos2::new(2.0, center_y - 20.0),
    ];

    normalize_points(rect, &mut points);

    // The peak deviation (80.0) is pulled in to half the height (50.0).
    assert!((points[1].y - (center_y + 50.0)).abs() < f32::EPSILON);
    assert!((points[0].y - (center_y - 25.0)).abs() < f32::EPSILON);
}

#[test]
fn leaves_peaks_inside_the_view() {
    let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(4.0, 100.0));
    let center_y = rect.center().y;
    let original = vec![
        Pos2::new(0.0, center_y - 10.0),
        Pos2::new(1.0, center_y + 20.0),
        Pos2::new(2.0, center_y - 5.0),
    ];
    let mut points = original.clone();

    normalize_points(rect, &mut points);

    assert_eq!(points, original);
}

#[test]
fn unipolar_maps_zero_to_bottom_one_to_top() {
    let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(100.0, 50.0));
    assert!((sample_to_y(rect, 0.0, false) - rect.bottom()).abs() < f32::EPSILON);
    assert!((sample_to_y(rect, 1.0, false) - rect.top()).abs() < f32::EPSILON);
}

#[test]
fn close_period_endpoints_match() {
    let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(10.0, 100.0));
    let waveform = [0.5f32, -0.2, -0.8, 0.1];
    let points = build_curve_points(rect, &waveform, true, true);

    assert!((points[0].y - points[points.len() - 1].y).abs() < f32::EPSILON);
}

#[test]
fn open_period_omits_wrap() {
    let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(10.0, 100.0));
    let waveform = [0.5f32, -0.2, -0.8, 0.1];
    let points = build_curve_points(rect, &waveform, true, false);

    assert_eq!(points.len(), waveform.len());
    let last_t = (waveform.len() - 1) as f32 / waveform.len() as f32;
    assert!((points.last().unwrap().x - (rect.left() + last_t * rect.width())).abs() < 1e-5);
}
