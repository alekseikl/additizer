use super::*;

fn approx(a: Smp, b: Smp) -> bool {
    (a - b).abs() < 1e-4
}

fn rate_slider(value: &mut Smp) -> Slider<'_> {
    Slider::mono(value, (1.0 / 50.0)..=50.0, Some(-50.0)).logarithmic()
}

#[test]
fn logarithmic_rate_places_one_hertz_at_midpoint() {
    let mut value = 1.0;
    let slider = rate_slider(&mut value);

    assert!(approx(slider.value_to_normalized(1.0), 0.5));
    assert!(approx(slider.value_to_normalized(50.0), 1.0));
    assert!(approx(slider.value_to_normalized(1.0 / 50.0), 0.0));

    assert!(approx(slider.normalized_to_value(0.5), 1.0));
    assert!(approx(slider.normalized_to_value(1.0), 50.0));
    assert!(approx(slider.normalized_to_value(0.0), 1.0 / 50.0));
}

#[test]
fn logarithmic_rate_keeps_equal_ratios_equally_spaced() {
    let mut value = 1.0;
    let slider = rate_slider(&mut value);

    let low = slider.value_to_normalized(2.0) - slider.value_to_normalized(1.0);
    let high = slider.value_to_normalized(20.0) - slider.value_to_normalized(10.0);
    assert!(approx(low, high));
}

#[test]
fn logarithmic_rate_mirrors_below_zero() {
    let mut value = 1.0;
    let slider = rate_slider(&mut value);

    assert!(approx(slider.value_to_normalized(-1.0), -0.5));
    assert!(approx(slider.value_to_normalized(-50.0), -1.0));
    assert!(approx(slider.normalized_to_value(-0.5), -1.0));
    assert!(approx(slider.normalized_to_value(-1.0), -50.0));
}
