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
    let level = ramp.value_at(phase, None, None);
    let pos = SegmentsEditor::playhead_pos(plot(), phase, level);

    assert_eq!(
        SegmentsEditor::playhead_pos(plot(), 0.0, ramp.value_at(0.0, None, None)),
        editor.points[0]
    );
    assert_eq!(
        SegmentsEditor::playhead_pos(plot(), 1.0, ramp.value_at(1.0, None, None)),
        *editor.points.last().unwrap()
    );
    assert!((pos.x - phase * 100.0).abs() < 1e-3);
    assert!((pos.y - (100.0 - level * 100.0)).abs() < 1e-4);
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

    SegmentsEditor::open().move_point(&mut open, 0, 0.0, 0.35, false);

    assert!((open.point(0).y - 0.35).abs() < 1e-6);
    assert_eq!(open.point(2).y, 0.0);

    SegmentsEditor::open().move_point(&mut open, 2, 1.0, 0.1, false);

    assert!((open.point(0).y - 0.35).abs() < 1e-6);
    assert!((open.point(2).y - 0.1).abs() < 1e-6);

    let mut closed = Segments::from_points(&[(0.0, 0.0), (0.5, 1.0), (1.0, 0.0)], false);
    let editor = SegmentsEditor::default();

    editor.move_point(&mut closed, 0, 0.0, 0.35, false);

    assert!((closed.point(0).y - 0.35).abs() < 1e-6);
    assert!((closed.point(2).y - 0.35).abs() < 1e-6);

    editor.move_point(&mut closed, 2, 1.0, 0.1, false);

    assert!((closed.point(0).y - 0.1).abs() < 1e-6);
    assert!((closed.point(2).y - 0.1).abs() < 1e-6);

    let mut mismatched = Segments::from_points(&[(0.0, 0.2), (0.5, 1.0), (1.0, 0.8)], false);

    editor.move_point(&mut mismatched, 2, 1.0, 0.4, false);

    assert!((mismatched.point(0).y - 0.4).abs() < 1e-6);
    assert!((mismatched.point(2).y - 0.4).abs() < 1e-6);
}

#[test]
fn curve_pos_continues_outside_the_plot() {
    let plot = plot();
    let above = SegmentsEditor::curve_pos(plot, pos2(50.0, -20.0));
    let outside = SegmentsEditor::curve_pos(plot, pos2(150.0, 120.0));

    assert!((above.x - 0.5).abs() < 1e-4);
    assert!((above.y - 1.2).abs() < 1e-4);
    assert!((outside.x - 1.5).abs() < 1e-4);
    assert!((outside.y + 0.2).abs() < 1e-4);
}

#[test]
fn bend_follows_the_pointer() {
    let editor = SegmentsEditor::default();
    let mut rising = Segments::from_points(&[(0.0, 0.0), (1.0, 1.0)], false);

    editor.move_power(&mut rising, 0, 0.1, false);

    assert!((rising.power(0) + 0.9).abs() < 1e-5);

    let mut falling = Segments::from_points(&[(0.0, 1.0), (1.0, 0.0)], false);

    editor.move_power(&mut falling, 0, 0.1, false);

    assert!((falling.power(0) - 0.9).abs() < 1e-5);

    let mut flat = Segments::from_points(&[(0.0, 0.5), (1.0, 0.5)], false);

    editor.move_power(&mut flat, 0, 0.1, false);

    assert_eq!(flat.power(0), 0.0);

    editor.move_power(&mut rising, 0, -3.0, false);

    assert_eq!(rising.power(0), addi_dsp::segments::MAX_POWER);
}

#[test]
fn synced_bend_mirrors_onto_the_next_segment() {
    let mut segments = Segments::from_points(&[(0.0, 0.0), (0.5, 1.0), (1.0, 0.2)], false);
    let editor = SegmentsEditor::default();

    editor.move_power(&mut segments, 0, -0.1, false);

    assert!((segments.power(0) - 0.9).abs() < 1e-5);
    assert_eq!(segments.power(1), 0.0);
    assert_eq!(segments.power(2), 0.0);

    editor.move_power(&mut segments, 0, -0.1, true);

    let power = segments.power(0);

    assert!((power - 1.8).abs() < 1e-5);
    assert!((segments.power(1) + power).abs() < 1e-5);
    assert_eq!(segments.power(2), 0.0);

    let before = segments.power(1);

    editor.move_power(&mut segments, 1, -0.05, true);

    assert!(segments.power(1) < before);
    assert!((segments.power(2) + segments.power(1)).abs() < 1e-5);
    assert!((segments.power(0) - power).abs() < 1e-5);

    editor.move_power(&mut segments, 2, 0.02, true);

    assert!((segments.power(0) + segments.power(2)).abs() < 1e-5);
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
