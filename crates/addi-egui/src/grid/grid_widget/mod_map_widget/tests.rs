use super::ModMapWidget;

#[test]
fn amount_scales_the_curve() {
    assert!((ModMapWidget::map_control(0.5, 1.0) - 0.5).abs() < 1e-6);
    assert!((ModMapWidget::map_control(0.5, 0.5) - 0.25).abs() < 1e-6);
    assert!((ModMapWidget::map_control(1.0, 0.0)).abs() < 1e-6);
    assert!((ModMapWidget::map_control(1.0, 1.5) - 1.0).abs() < 1e-6);
}
