use egui::{Rect, pos2};

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

    assert!(points.len() >= 101);
    assert_eq!(points[0], pos2(0.0, 100.0));
    assert_eq!(*points.last().unwrap(), pos2(100.0, 0.0));

    let at_40 = points
        .iter()
        .find(|point| (point.x - 40.0).abs() < 1e-3)
        .unwrap();

    assert!((at_40.y - 60.0).abs() < 1e-4);
}

#[test]
fn playhead_follows_the_rendered_polyline() {
    let ramp = Segments::from_points(&[(0.0, 0.0), (1.0, 1.0)], false);
    let editor = curve(&ramp);
    let phase = 0.405;
    let pos = SegmentsEditor::curve_pos(plot(), &ramp, phase);

    assert_eq!(
        SegmentsEditor::curve_pos(plot(), &ramp, 0.0),
        editor.points[0]
    );
    assert_eq!(
        SegmentsEditor::curve_pos(plot(), &ramp, 1.0),
        *editor.points.last().unwrap()
    );
    assert!((pos.x - phase * 100.0).abs() < 1e-3);
    assert!((pos.y - (100.0 - ramp.value_at(phase) * 100.0)).abs() < 1e-4);
}

#[test]
fn square_corners_are_vertical() {
    let editor = curve(&segment_presets::square());
    let points = &editor.points;
    let at = |x: f32| -> Vec<f32> {
        points
            .iter()
            .filter(|point| (point.x - x).abs() < 1e-3)
            .map(|point| point.y)
            .collect()
    };

    assert_eq!(at(0.0).first().copied(), Some(100.0));
    assert_eq!(at(0.0).last().copied(), Some(0.0));
    assert_eq!(at(50.0).first().copied(), Some(0.0));
    assert_eq!(at(50.0).last().copied(), Some(100.0));
    assert_eq!(*points.last().unwrap(), pos2(100.0, 100.0));
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
