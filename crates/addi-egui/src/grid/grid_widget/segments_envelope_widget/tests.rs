use addi_dsp::Segments;
use egui::vec2;

use super::SegmentsEnvelopeWidget;

fn size() -> egui::Vec2 {
    vec2(3.0, 10.0)
}

#[test]
fn curve_keeps_an_off_grid_peak() {
    let segments = Segments::from_points(&[(0.0, 0.0), (0.5, 1.0), (1.0, 0.0)], false);
    let mut widget = SegmentsEnvelopeWidget::default();

    widget.build_curve(&segments, size());

    assert!(
        widget
            .points
            .iter()
            .any(|point| { (point[0] - 0.5).abs() < 1e-6 && (point[1] - 1.0).abs() < 1e-6 })
    );
    assert!(
        widget
            .points
            .windows(2)
            .all(|pair| pair[1][0] + 1e-5 >= pair[0][0])
    );
}

#[test]
fn intact_segments_are_not_rendered_again() {
    let segments = Segments::from_points(&[(0.0, 0.0), (0.5, 1.0), (1.0, 0.0)], false);
    let mut widget = SegmentsEnvelopeWidget::default();

    widget.build_curve(&segments, size());
    widget.scratch.resize(9999, 1.0);
    widget.build_curve(&segments, size());

    assert_eq!(widget.scratch.len(), 9999);
}

#[test]
fn an_edit_renders_again() {
    let mut segments = Segments::from_points(&[(0.0, 0.0), (0.5, 1.0), (1.0, 0.0)], false);
    let mut widget = SegmentsEnvelopeWidget::default();

    widget.build_curve(&segments, size());
    widget.scratch.resize(9999, 1.0);
    segments.set_power(0, 2.0);
    widget.build_curve(&segments, size());

    assert_eq!(widget.scratch.len(), 4);
}

#[test]
fn a_wider_tile_renders_again() {
    let segments = Segments::from_points(&[(0.0, 0.0), (0.5, 1.0), (1.0, 0.0)], false);
    let mut widget = SegmentsEnvelopeWidget::default();

    widget.build_curve(&segments, size());
    widget.scratch.resize(9999, 1.0);
    widget.build_curve(&segments, vec2(8.0, 10.0));

    assert_eq!(widget.scratch.len(), 9);
}
