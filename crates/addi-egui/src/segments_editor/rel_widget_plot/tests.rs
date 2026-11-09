use addi_dsp::RelSegments;
use egui::vec2;

use super::{CurveMap, RelWidgetPlot};

fn level(y: f32, amount: f32, bipolar: bool) -> f32 {
    super::level(y, Some(CurveMap { amount, bipolar }))
}

fn size() -> egui::Vec2 {
    vec2(3.0, 10.0)
}

#[test]
fn curve_keeps_an_off_grid_peak() {
    let segments = RelSegments::from_points(&[(0.0, 0.0), (0.5, 1.0), (1.0, 0.0)], false);
    let mut plot = RelWidgetPlot::lfo();

    plot.build_curve(&segments, size());

    assert!(
        plot.curve
            .iter()
            .any(|point| { (point[0] - 0.5).abs() < 1e-6 && (point[1] - 1.0).abs() < 1e-6 })
    );
    assert!(
        plot.curve
            .array_windows()
            .all(|&[prev, next]| next[0] + 1e-5 >= prev[0])
    );
}

#[test]
fn intact_segments_are_not_rendered_again() {
    let segments = RelSegments::from_points(&[(0.0, 0.0), (0.5, 1.0), (1.0, 0.0)], false);
    let mut plot = RelWidgetPlot::lfo();

    plot.build_curve(&segments, size());
    plot.scratch.resize(9999, 1.0);
    plot.build_curve(&segments, size());

    assert_eq!(plot.scratch.len(), 9999);
}

#[test]
fn an_edit_renders_again() {
    let mut segments = RelSegments::from_points(&[(0.0, 0.0), (0.5, 1.0), (1.0, 0.0)], false);
    let mut plot = RelWidgetPlot::lfo();

    plot.build_curve(&segments, size());
    plot.scratch.resize(9999, 1.0);
    segments.set_power(0, 2.0);
    plot.build_curve(&segments, size());

    assert_eq!(plot.scratch.len(), 4);
}

#[test]
fn a_wider_tile_renders_again() {
    let segments = RelSegments::from_points(&[(0.0, 0.0), (0.5, 1.0), (1.0, 0.0)], false);
    let mut plot = RelWidgetPlot::lfo();

    plot.build_curve(&segments, size());
    plot.scratch.resize(9999, 1.0);
    plot.build_curve(&segments, vec2(8.0, 10.0));

    assert_eq!(plot.scratch.len(), 9);
}

#[test]
fn amount_scales_the_curve() {
    assert!((level(0.5, 1.0, false) - 0.5).abs() < 1e-6);
    assert!((level(0.5, 0.5, false) - 0.25).abs() < 1e-6);
    assert!((level(1.0, 0.0, false)).abs() < 1e-6);
    assert!((level(1.0, 1.5, false) - 1.0).abs() < 1e-6);
}

#[test]
fn bipolar_folds_before_amount() {
    assert!((level(0.5, 1.0, true)).abs() < 1e-6);
    assert!((level(0.0, 1.0, true) + 1.0).abs() < 1e-6);
    assert!((level(1.0, 1.0, true) - 1.0).abs() < 1e-6);
    assert!((level(1.0, 0.5, true) - 0.5).abs() < 1e-6);
    assert!((level(0.0, 0.5, true) + 0.5).abs() < 1e-6);
    assert!((level(0.5, 0.5, true)).abs() < 1e-6);
}
