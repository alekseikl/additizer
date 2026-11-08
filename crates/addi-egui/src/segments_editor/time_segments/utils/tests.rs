use super::{drag_span, zoom_drag, zoom_span};

#[test]
fn dragging_past_the_edge_grows_the_window() {
    assert!((drag_span(1.0, 0.4) - 1.0).abs() < 1e-6);
    assert!((drag_span(1.0, 1.6) - 1.6).abs() < 1e-6);
}

#[test]
fn positive_amount_zooms_in() {
    let wider = zoom_span(4.0, -super::C::ZOOM_POINTS);

    assert!((wider - 8.0).abs() < 1e-4);
    assert!((zoom_span(0.2, 800.0) - 0.125).abs() < 1e-4);
    assert!(zoom_span(50.0, -800.0) <= 60.0);
}

#[test]
fn dragging_the_glass_up_zooms_in() {
    let closer = zoom_drag(4.0, -super::C::ZOOM_POINTS);
    let wider = zoom_drag(4.0, super::C::ZOOM_POINTS);

    assert!((closer - 2.0).abs() < 1e-4);
    assert!((wider - 8.0).abs() < 1e-4);
}
