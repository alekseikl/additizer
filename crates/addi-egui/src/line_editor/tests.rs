use egui::{Pos2, Rect, pos2};

use addi_dsp::Segments;

use crate::segment_presets;

use super::SegmentsEditor;

fn plot() -> Rect {
    Rect::from_min_max(pos2(0.0, 0.0), pos2(100.0, 100.0))
}

fn curve(segments: &Segments) -> SegmentsEditor {
    let mut editor = SegmentsEditor::default();
    editor.curve_points(plot(), segments, 1.0);
    editor
}

#[test]
fn linear_segment_keeps_a_point_per_pixel() {
    let ramp = Segments::from_points(&[(0.0, 0.0), (1.0, 1.0)], false);
    let editor = curve(&ramp);
    let points = &editor.points;

    assert_eq!(points.len(), 101);
    assert_eq!(points[0], pos2(0.0, 100.0));
    assert_eq!(points[40].x, 40.0);
    assert!((points[40].y - 60.0).abs() < 1e-4);
    assert_eq!(points[100], pos2(100.0, 0.0));
}

#[test]
fn playhead_follows_the_rendered_polyline() {
    let ramp = Segments::from_points(&[(0.0, 0.0), (1.0, 1.0)], false);
    let editor = curve(&ramp);
    let phase = 0.405;
    let pos = editor.curve_pos(phase);
    let from = editor.points[40];
    let to = editor.points[41];
    let frac = (pos.x - from.x) / (to.x - from.x);

    assert_eq!(editor.curve_pos(0.0), editor.points[0]);
    assert_eq!(editor.curve_pos(1.0), *editor.points.last().unwrap());
    assert!((pos.x - phase * 100.0).abs() < 1e-3);
    assert!(pos.x > from.x && pos.x < to.x);
    assert!((pos.y - (from.y + (to.y - from.y) * frac)).abs() < 1e-4);
    assert_eq!(SegmentsEditor::default().curve_pos(0.5), Pos2::ZERO);
}

#[test]
fn square_steps_between_adjacent_pixels() {
    let editor = curve(&segment_presets::square());
    let points = &editor.points;

    assert_eq!(points[0], pos2(0.0, 0.0));
    assert_eq!(points[50], pos2(50.0, 0.0));
    assert_eq!(points[51], pos2(51.0, 100.0));
    assert_eq!(points[points.len() - 1], pos2(100.0, 100.0));
}

#[test]
fn smooth_curve_keeps_intermediate_pixels() {
    let editor = curve(&segment_presets::sine());
    let points = &editor.points;

    assert!(points.len() > 3);
    assert_eq!(points[0], pos2(0.0, 100.0));
    assert!(points.contains(&pos2(50.0, 0.0)));
    assert_eq!(points[points.len() - 1], pos2(100.0, 100.0));
}
