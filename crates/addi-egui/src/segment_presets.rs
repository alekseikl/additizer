use addi_dsp::{Segments, Smp};

pub fn triangle() -> Segments {
    triangle_at(0.5)
}

pub fn square() -> Segments {
    Segments::from_points(
        &[(0.0, 0.0), (0.0, 1.0), (0.5, 1.0), (0.5, 0.0), (1.0, 0.0)],
        false,
    )
}

pub fn sine() -> Segments {
    sine_at(0.5)
}

/// Rising ramp that drops back to `0` at the loop point.
pub fn saw_up() -> Segments {
    Segments::from_points(&[(0.0, 0.0), (1.0, 1.0), (1.0, 0.0)], false)
}

/// Falling ramp that jumps back to `1` at the loop point.
pub fn saw_down() -> Segments {
    Segments::from_points(&[(0.0, 1.0), (1.0, 0.0), (1.0, 1.0)], false)
}

/// Triangle whose peak sits at `skew`.
pub fn triangle_at(skew: Smp) -> Segments {
    let skew = skew.clamp(0.0, 1.0);

    if skew <= 0.0 {
        return Segments::from_points(&[(0.0, 1.0), (1.0, 0.0)], false);
    }

    if skew >= 1.0 {
        return Segments::from_points(&[(0.0, 0.0), (1.0, 1.0)], false);
    }

    Segments::from_points(&[(0.0, 0.0), (skew, 1.0), (1.0, 0.0)], false)
}

/// Smooth triangle peaked at `skew`.
pub fn sine_at(skew: Smp) -> Segments {
    let mut segments = triangle_at(skew);

    for index in 0..segments.num_points() {
        segments.set_smooth(index, true);
    }

    segments
}
