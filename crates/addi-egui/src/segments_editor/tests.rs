use egui::{Context, Id, Rect, Ui, UiBuilder, pos2};

use addi_dsp::{Segments, segments::MAX_POINTS};

use crate::segment_presets;

use super::{Pattern, SegmentsEditor, Tool};

fn plot() -> Rect {
    Rect::from_min_max(pos2(0.0, 0.0), pos2(100.0, 100.0))
}

fn press(shift: bool) -> Ui {
    let ctx = Context::default();

    ctx.input_mut(|input| input.modifiers.shift = shift);
    Ui::new(ctx, Id::new("segments-test"), UiBuilder::new())
}

fn curve(segments: &Segments) -> SegmentsEditor {
    let mut editor = SegmentsEditor::default();
    editor.build_curve_points(plot(), segments);
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
    editor.build_curve_points(plot(), &ramp);

    assert_eq!(editor.scratch.len(), 9999);
    assert_eq!(editor.points, points);
}

#[test]
fn a_moved_plot_reuses_the_curve() {
    let ramp = Segments::from_points(&[(0.0, 0.0), (1.0, 1.0)], false);
    let mut editor = curve(&ramp);
    let moved = Rect::from_min_max(pos2(10.0, 20.0), pos2(110.0, 120.0));

    editor.scratch.resize(9999, 1.0);
    editor.build_curve_points(moved, &ramp);

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
    editor.build_curve_points(plot(), &ramp);

    assert_eq!(editor.scratch.len(), 101);
}

#[test]
fn a_repeated_write_renders_again() {
    let mut ramp = Segments::from_points(&[(0.0, 0.0), (1.0, 1.0)], false);
    let mut editor = curve(&ramp);

    ramp.set_power(0, 0.0);
    editor.scratch.resize(9999, 1.0);
    editor.build_curve_points(plot(), &ramp);

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
    editor.build_curve_points(wider, &ramp);

    assert_eq!(editor.scratch.len(), 181);
    assert_eq!(*editor.points.last().unwrap(), pos2(180.0, 0.0));
}

#[test]
fn changed_segments_render_again() {
    let ramp = Segments::from_points(&[(0.0, 0.0), (1.0, 1.0)], false);
    let mut editor = curve(&ramp);
    let flat = Segments::from_points(&[(0.0, 0.5), (1.0, 0.5)], false);

    editor.scratch.resize(9999, 1.0);
    editor.build_curve_points(plot(), &flat);

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
    let ui = press(true);

    SegmentsEditor::lookup().move_point(&ui, &mut open, 0, 0.0, 0.35);

    assert!((open.point(0).y - 0.35).abs() < 1e-6);
    assert_eq!(open.point(2).y, 0.0);

    SegmentsEditor::lookup().move_point(&ui, &mut open, 2, 1.0, 0.1);

    assert!((open.point(0).y - 0.35).abs() < 1e-6);
    assert!((open.point(2).y - 0.1).abs() < 1e-6);

    let mut closed = Segments::from_points(&[(0.0, 0.0), (0.5, 1.0), (1.0, 0.0)], false);
    let editor = SegmentsEditor::default();

    editor.move_point(&ui, &mut closed, 0, 0.0, 0.35);

    assert!((closed.point(0).y - 0.35).abs() < 1e-6);
    assert!((closed.point(2).y - 0.35).abs() < 1e-6);

    editor.move_point(&ui, &mut closed, 2, 1.0, 0.1);

    assert!((closed.point(0).y - 0.1).abs() < 1e-6);
    assert!((closed.point(2).y - 0.1).abs() < 1e-6);

    let mut mismatched = Segments::from_points(&[(0.0, 0.2), (0.5, 1.0), (1.0, 0.8)], false);

    editor.move_point(&ui, &mut mismatched, 2, 1.0, 0.4);

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
    let mut editor = SegmentsEditor::default();
    let mut rising = Segments::from_points(&[(0.0, 0.0), (1.0, 1.0)], false);
    let ui = press(false);

    editor.move_power(&ui, &mut rising, 0, 0.1);

    assert!((rising.power(0) + 0.9).abs() < 1e-5);

    editor.move_power(&ui, &mut rising, 0, 0.0);

    assert_eq!(rising.power(0), 0.0);

    let mut falling = Segments::from_points(&[(0.0, 1.0), (1.0, 0.0)], false);

    editor.move_power(&ui, &mut falling, 0, 0.1);

    assert!((falling.power(0) - 0.9).abs() < 1e-5);

    let mut flat = Segments::from_points(&[(0.0, 0.5), (1.0, 0.5)], false);

    editor.move_power(&ui, &mut flat, 0, 0.1);

    assert_eq!(flat.power(0), 0.0);

    editor.drag_power = rising.power(0);
    editor.move_power(&ui, &mut rising, 0, -3.0);

    assert_eq!(rising.power(0), addi_dsp::segments::MAX_POWER);
}

#[test]
fn synced_bend_mirrors_onto_the_next_segment() {
    let mut segments = Segments::from_points(&[(0.0, 0.0), (0.5, 1.0), (1.0, 0.2)], false);
    let mut editor = SegmentsEditor::default();
    let ui = press(false);

    editor.move_power(&ui, &mut segments, 0, -0.1);

    assert!((segments.power(0) - 0.9).abs() < 1e-5);
    assert_eq!(segments.power(1), 0.0);
    assert_eq!(segments.power(2), 0.0);

    editor.drag_power = segments.power(0);

    let shift = press(true);

    editor.move_power(&shift, &mut segments, 0, -0.1);

    let power = segments.power(0);

    assert!((power - 1.8).abs() < 1e-5);
    assert!((segments.power(1) + power).abs() < 1e-5);
    assert_eq!(segments.power(2), 0.0);

    let before = segments.power(1);

    editor.drag_power = before;
    editor.move_power(&shift, &mut segments, 1, -0.05);

    assert!(segments.power(1) < before);
    assert!((segments.power(2) + segments.power(1)).abs() < 1e-5);
    assert!((segments.power(0) - power).abs() < 1e-5);

    editor.drag_power = segments.power(2);
    editor.move_power(&shift, &mut segments, 2, 0.02);

    assert!((segments.power(0) + segments.power(2)).abs() < 1e-5);
}

fn levels(segments: &Segments) -> Vec<(f32, f32)> {
    segments
        .points()
        .iter()
        .map(|point| (point.x, point.y))
        .collect()
}

#[test]
fn paint_stamps_the_shape_into_one_column() {
    let mut ramp = Segments::from_points(&[(0.0, 0.0), (1.0, 1.0)], false);

    SegmentsEditor::paint_cell(&mut ramp, 0.25, 0.5, 1.0, Pattern::Down);

    assert_eq!(
        levels(&ramp),
        vec![
            (0.0, 0.0),
            (0.25, 0.25),
            (0.25, 1.0),
            (0.5, 0.0),
            (0.5, 0.5),
            (1.0, 1.0),
        ]
    );
    assert!(!ramp.point(2).smooth);

    let mut bent = Segments::from_points(&[(0.0, 0.0), (1.0, 1.0)], false);

    bent.set_power(0, 1.5);
    bent.set_smooth(0, true);

    let from_y = bent.value_at(0.25, None, None);
    let to_y = bent.value_at(0.5, None, None);

    SegmentsEditor::paint_cell(&mut bent, 0.25, 0.5, 1.0, Pattern::Down);

    assert_eq!(bent.point(0).power, 1.5);
    assert!(bent.point(0).smooth);
    assert_eq!(bent.point(1).power, 0.0);
    assert!((bent.point(1).y - from_y).abs() < 1e-5);
    assert!((bent.point(4).y - to_y).abs() < 1e-5);

    let outside = Segments::from_points(&[(0.0, 0.0), (0.75, 0.3), (1.0, 1.0)], false);
    let mut outside = outside;

    outside.set_power(1, 4.0);
    SegmentsEditor::paint_cell(&mut outside, 0.25, 0.5, 1.0, Pattern::Up);

    let kept = outside
        .points()
        .iter()
        .find(|point| point.x == 0.75)
        .copied()
        .unwrap();

    assert_eq!(kept.y, 0.3);
    assert_eq!(kept.power, 4.0);

    let mut pulse = Segments::from_points(&[(0.0, 0.0), (1.0, 1.0)], false);

    SegmentsEditor::paint_cell(&mut pulse, 0.0, 0.5, 0.5, Pattern::Half);

    assert_eq!(
        levels(&pulse),
        vec![
            (0.0, 0.0),
            (0.0, 0.5),
            (0.25, 0.5),
            (0.25, 0.0),
            (0.5, 0.0),
            (0.5, 0.5),
            (1.0, 1.0),
        ]
    );
}

#[test]
fn painting_the_same_column_again_leaves_it() {
    let mut ramp = Segments::from_points(&[(0.0, 0.0), (1.0, 1.0)], false);

    SegmentsEditor::paint_cell(&mut ramp, 0.25, 0.5, 1.0, Pattern::Tri);

    let revision = ramp.revision();
    let points = ramp.points().to_vec();

    SegmentsEditor::paint_cell(&mut ramp, 0.25, 0.5, 1.0, Pattern::Tri);

    assert_eq!(ramp.revision(), revision);
    assert_eq!(ramp.points(), points.as_slice());
}

#[test]
fn paint_keeps_markers_outside_the_column() {
    let mut segments =
        Segments::from_points(&[(0.0, 0.0), (0.3, 0.4), (0.8, 0.6), (1.0, 1.0)], false);

    assert!(segments.set_loop_point(Some(1)));
    assert!(segments.set_release_point(Some(3)));
    SegmentsEditor::paint_cell(&mut segments, 0.25, 0.5, 1.0, Pattern::Down);

    assert!(segments.loop_point_idx().is_none());
    assert_eq!(segments.release_point().map(|point| point.x), Some(1.0));

    let mut kept = Segments::from_points(&[(0.0, 0.0), (0.25, 0.4), (0.8, 0.6), (1.0, 1.0)], false);

    assert!(kept.set_loop_point(Some(1)));
    assert!(kept.set_release_point(Some(2)));
    SegmentsEditor::paint_cell(&mut kept, 0.25, 0.5, 1.0, Pattern::Step);

    assert_eq!(
        kept.loop_point().map(|point| (point.x, point.y)),
        Some((0.25, 0.4))
    );
    assert_eq!(kept.release_point().map(|point| point.x), Some(0.8));
}

#[test]
fn paint_stops_at_the_point_limit() {
    let mut points = Vec::with_capacity(MAX_POINTS);

    points.push((0.0, 0.0));

    for _ in 1..MAX_POINTS - 1 {
        points.push((0.5, 0.25));
    }

    points.push((1.0, 1.0));

    let mut full = Segments::from_points(&points, false);
    let before = full.points().to_vec();

    SegmentsEditor::paint_cell(&mut full, 0.0, 0.25, 1.0, Pattern::Half);

    assert_eq!(full.points(), before.as_slice());
}

#[test]
fn paint_snaps_the_level_to_the_vertical_grid() {
    let mut editor = SegmentsEditor::default();
    let plot = plot();
    let mut flat = Segments::from_points(&[(0.0, 0.0), (1.0, 0.0)], false);

    editor.grid_x = 1;
    editor.grid_y = 4;
    editor.tool = Tool::Paint(Pattern::Step);
    editor.paint_at(&press(false), plot, &mut flat, pos2(10.0, 48.0));

    assert_eq!(
        levels(&flat),
        vec![(0.0, 0.0), (0.0, 0.5), (1.0, 0.5), (1.0, 0.0)]
    );

    let mut free = Segments::from_points(&[(0.0, 0.0), (1.0, 0.0)], false);

    editor.paint_at(&press(true), plot, &mut free, pos2(10.0, 48.0));

    let high = free.point(1).y;

    assert!((high - 0.52).abs() < 1e-5);
    assert!((free.point(2).y - high).abs() < 1e-5);
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
