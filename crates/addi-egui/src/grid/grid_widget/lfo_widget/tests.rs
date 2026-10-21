use addi_dsp::Segments;

use super::LfoWidget;

#[test]
fn curve_keeps_an_off_grid_peak() {
    let segments = Segments::from_points(&[(0.0, 0.0), (0.5, 1.0), (1.0, 0.0)], false);
    let mut widget = LfoWidget::default();

    widget.build_curve(&segments, 3);

    assert!(
        widget
            .points
            .iter()
            .any(|point| { (point.x - 0.5).abs() < 1e-6 && (point.y - 1.0).abs() < 1e-6 })
    );
    assert!(
        widget
            .points
            .windows(2)
            .all(|pair| pair[1].x + 1e-5 >= pair[0].x)
    );
}
