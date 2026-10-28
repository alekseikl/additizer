use super::ModMapWidget;

#[test]
fn amount_scales_the_curve() {
    assert!((ModMapWidget::map_control(0.5, 1.0, false) - 0.5).abs() < 1e-6);
    assert!((ModMapWidget::map_control(0.5, 0.5, false) - 0.25).abs() < 1e-6);
    assert!((ModMapWidget::map_control(1.0, 0.0, false)).abs() < 1e-6);
    assert!((ModMapWidget::map_control(1.0, 1.5, false) - 1.0).abs() < 1e-6);
}

#[test]
fn bipolar_folds_before_amount() {
    assert!((ModMapWidget::map_control(0.5, 1.0, true)).abs() < 1e-6);
    assert!((ModMapWidget::map_control(0.0, 1.0, true) + 1.0).abs() < 1e-6);
    assert!((ModMapWidget::map_control(1.0, 1.0, true) - 1.0).abs() < 1e-6);
    assert!((ModMapWidget::map_control(1.0, 0.5, true) - 0.5).abs() < 1e-6);
    assert!((ModMapWidget::map_control(0.0, 0.5, true) + 0.5).abs() < 1e-6);
    assert!((ModMapWidget::map_control(0.5, 0.5, true)).abs() < 1e-6);
}
