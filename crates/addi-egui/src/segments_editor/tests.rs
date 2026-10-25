use egui::{Rect, pos2};

use addi_dsp::Segments;

use crate::segment_presets;

use super::SegmentsEditor;

fn plot() -> Rect {
    Rect::from_min_max(pos2(0.0, 0.0), pos2(100.0, 100.0))
}

fn curve(segments: &Segments) -> SegmentsEditor {
    let mut editor = SegmentsEditor::default();
    editor.curve_points(plot(), segments);
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
fn intact_segments_are_not_rendered_again() {
    let ramp = Segments::from_points(&[(0.0, 0.0), (1.0, 1.0)], false);
    let mut editor = curve(&ramp);
    let points = editor.points.clone();

    editor.scratch.resize(9999, 1.0);
    editor.curve_points(plot(), &ramp);

    assert_eq!(editor.scratch.len(), 9999);
    assert_eq!(editor.points, points);
}

#[test]
fn a_moved_plot_reuses_the_curve() {
    let ramp = Segments::from_points(&[(0.0, 0.0), (1.0, 1.0)], false);
    let mut editor = curve(&ramp);
    let moved = Rect::from_min_max(pos2(10.0, 20.0), pos2(110.0, 120.0));

    editor.scratch.resize(9999, 1.0);
    editor.curve_points(moved, &ramp);

    assert_eq!(editor.scratch.len(), 9999);
    assert_eq!(editor.points[0], pos2(10.0, 120.0));
    assert_eq!(*editor.points.last().unwrap(), pos2(110.0, 20.0));
}

#[test]
fn an_edit_renders_again() {
    let mut ramp = Segments::from_points(&[(0.0, 0.0), (1.0, 1.0)], false);
    let mut editor = curve(&ramp);

    editor.scratch.resize(9999, 1.0);
    ramp.set_power(0, 2.0);
    editor.curve_points(plot(), &ramp);

    assert_eq!(editor.scratch.len(), 101);
}

#[test]
fn a_repeated_write_renders_again() {
    let mut ramp = Segments::from_points(&[(0.0, 0.0), (1.0, 1.0)], false);
    let mut editor = curve(&ramp);

    ramp.set_power(0, 0.0);
    editor.scratch.resize(9999, 1.0);
    editor.curve_points(plot(), &ramp);

    assert_eq!(editor.scratch.len(), 101);
    assert_eq!(editor.points[0], pos2(0.0, 100.0));
    assert_eq!(*editor.points.last().unwrap(), pos2(100.0, 0.0));
}

#[test]
fn a_wider_plot_renders_again() {
    let ramp = Segments::from_points(&[(0.0, 0.0), (1.0, 1.0)], false);
    let mut editor = curve(&ramp);
    let wider = Rect::from_min_max(pos2(0.0, 0.0), pos2(180.0, 100.0));

    editor.scratch.resize(9999, 1.0);
    editor.curve_points(wider, &ramp);

    assert_eq!(editor.scratch.len(), 181);
    assert_eq!(*editor.points.last().unwrap(), pos2(180.0, 0.0));
}

#[test]
fn changed_segments_render_again() {
    let ramp = Segments::from_points(&[(0.0, 0.0), (1.0, 1.0)], false);
    let mut editor = curve(&ramp);
    let flat = Segments::from_points(&[(0.0, 0.5), (1.0, 0.5)], false);

    editor.scratch.resize(9999, 1.0);
    editor.curve_points(plot(), &flat);

    assert_eq!(editor.scratch.len(), 101);
    assert!(
        editor
            .points
            .iter()
            .all(|point| (point.y - 50.0).abs() < 1e-3)
    );
}

#[test]
fn open_curve_moves_the_ends_independently() {
    let mut open = Segments::from_points(&[(0.0, 0.0), (0.5, 1.0), (1.0, 0.0)], false);

    SegmentsEditor::move_point(&mut open, 0, 0.0, 0.35, false, 1, 1, false);

    assert!((open.segment(0).y - 0.35).abs() < 1e-6);
    assert_eq!(open.segment(2).y, 0.0);

    SegmentsEditor::move_point(&mut open, 2, 1.0, 0.1, false, 1, 1, false);

    assert!((open.segment(0).y - 0.35).abs() < 1e-6);
    assert!((open.segment(2).y - 0.1).abs() < 1e-6);

    let mut closed = Segments::from_points(&[(0.0, 0.0), (0.5, 1.0), (1.0, 0.0)], false);

    SegmentsEditor::move_point(&mut closed, 0, 0.0, 0.35, false, 1, 1, true);

    assert!((closed.segment(0).y - 0.35).abs() < 1e-6);
    assert!((closed.segment(2).y - 0.35).abs() < 1e-6);

    SegmentsEditor::move_point(&mut closed, 2, 1.0, 0.1, false, 1, 1, true);

    assert!((closed.segment(0).y - 0.1).abs() < 1e-6);
    assert!((closed.segment(2).y - 0.1).abs() < 1e-6);
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
