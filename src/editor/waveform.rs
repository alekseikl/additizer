use egui::{Color32, Mesh, Painter, Pos2, Rect, Shape, Stroke, ecolor::Hsva};

const DEFAULT_STROKE_COLOR: Hsva = Hsva {
    h: 0.03,
    s: 0.9,
    v: 1.0,
    a: 1.0,
};
const LINE_WIDTH: f32 = 1.0;

#[derive(Clone, Copy, Debug)]
pub struct WaveformOptions {
    /// When true, append a point at t = 1 that wraps to the first sample.
    pub close_period: bool,
    pub normalize: bool,
    /// Stroke / tint color for the waveform.
    pub color: Color32,
    /// When true, paint the under-curve fill (oscillator default).
    pub fill: bool,
    /// When true, samples are in [-1, 1] with zero at the center (default).
    /// When false, samples are in [0, 1] with zero at the bottom (full height).
    pub bipolar: bool,
}

impl Default for WaveformOptions {
    fn default() -> Self {
        Self {
            close_period: true,
            normalize: true,
            color: Color32::from(DEFAULT_STROKE_COLOR),
            fill: true,
            bipolar: true,
        }
    }
}

fn sample_to_y(rect: Rect, sample: f32, bipolar: bool) -> f32 {
    if bipolar {
        rect.center().y - sample * rect.height() * 0.5
    } else {
        rect.bottom() - sample.clamp(0.0, 1.0) * rect.height()
    }
}

fn build_curve_points(
    rect: Rect,
    waveform: &[f32],
    bipolar: bool,
    close_period: bool,
) -> Vec<Pos2> {
    let n = waveform.len();
    let t_mult = (n as f32).recip();
    let end = if close_period { n } else { n - 1 };
    let mut points = Vec::with_capacity(end + 1);

    for i in 0..=end {
        let t = i as f32 * t_mult;
        points.push(Pos2::new(
            rect.left() + t * rect.width(),
            sample_to_y(rect, waveform[i % n], bipolar),
        ));
    }

    points
}

/// Scale the curve's vertical deviation from the center so its peak fills the view.
fn normalize_points(rect: Rect, points: &mut [Pos2]) {
    let center_y = rect.center().y;
    let peak = points
        .iter()
        .fold(0.0_f32, |acc, p| acc.max((p.y - center_y).abs()));

    if peak <= 1e-6 {
        return;
    }

    let scale = rect.height() * 0.5 / peak;
    for p in points.iter_mut() {
        p.y = center_y + (p.y - center_y) * scale;
    }
}

fn with_alpha(color: Color32, alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha)
}

fn paint_fill(painter: &Painter, rect: Rect, points: &[Pos2], options: WaveformOptions) {
    if points.len() < 2 {
        return;
    }

    let baseline_y = if options.bipolar {
        rect.center().y
    } else {
        rect.bottom()
    };
    let mut mesh = Mesh::default();

    let fill_edge = with_alpha(options.color, 0x47);
    let fill_base = with_alpha(options.color, 0x66);

    let mut add_segment = |a: Pos2, b: Pos2| {
        let ca = Pos2::new(a.x, baseline_y);
        let cb = Pos2::new(b.x, baseline_y);

        let i_a = mesh.vertices.len() as u32;
        mesh.colored_vertex(a, fill_edge);
        let i_b = mesh.vertices.len() as u32;
        mesh.colored_vertex(b, fill_edge);
        let i_cb = mesh.vertices.len() as u32;
        mesh.colored_vertex(cb, fill_base);
        let i_ca = mesh.vertices.len() as u32;
        mesh.colored_vertex(ca, fill_base);

        mesh.add_triangle(i_a, i_b, i_cb);
        mesh.add_triangle(i_a, i_cb, i_ca);
    };

    for window in points.windows(2) {
        add_segment(window[0], window[1]);
    }

    painter.add(Shape::mesh(mesh));
}

fn paint_stroke(painter: &Painter, points: &[Pos2], options: WaveformOptions) {
    if points.len() < 2 {
        return;
    }

    painter.line(points.to_vec(), Stroke::new(LINE_WIDTH, options.color));
}

pub fn paint_waveform(painter: &Painter, rect: Rect, waveform: &[f32]) {
    paint_waveform_with_options(painter, rect, waveform, WaveformOptions::default());
}

pub fn paint_waveform_with_options(
    painter: &Painter,
    rect: Rect,
    waveform: &[f32],
    options: WaveformOptions,
) {
    if !rect.is_positive() || waveform.len() < 2 {
        return;
    }

    let mut points = build_curve_points(rect, waveform, options.bipolar, options.close_period);

    if options.normalize {
        normalize_points(rect, &mut points);
    }

    if options.fill {
        paint_fill(painter, rect, &points, options);
    }

    paint_stroke(painter, &points, options);
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::Vec2;

    #[test]
    fn normalizes_points_to_fill_height() {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(4.0, 100.0));
        let center_y = rect.center().y;
        let mut points = vec![
            Pos2::new(0.0, center_y - 10.0),
            Pos2::new(1.0, center_y + 20.0),
            Pos2::new(2.0, center_y - 5.0),
        ];

        normalize_points(rect, &mut points);

        // The peak deviation (20.0) should now reach half the height (50.0).
        assert!((points[1].y - (center_y + 50.0)).abs() < f32::EPSILON);
        assert!((points[0].y - (center_y - 25.0)).abs() < f32::EPSILON);
    }

    #[test]
    fn unipolar_maps_zero_to_bottom_one_to_top() {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(100.0, 50.0));
        assert!((sample_to_y(rect, 0.0, false) - rect.bottom()).abs() < f32::EPSILON);
        assert!((sample_to_y(rect, 1.0, false) - rect.top()).abs() < f32::EPSILON);
    }

    #[test]
    fn close_period_endpoints_match() {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(10.0, 100.0));
        let waveform = [0.5f32, -0.2, -0.8, 0.1];
        let points = build_curve_points(rect, &waveform, true, true);

        assert!((points[0].y - points[points.len() - 1].y).abs() < f32::EPSILON);
    }

    #[test]
    fn open_period_omits_wrap() {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(10.0, 100.0));
        let waveform = [0.5f32, -0.2, -0.8, 0.1];
        let points = build_curve_points(rect, &waveform, true, false);

        assert_eq!(points.len(), waveform.len());
        let last_t = (waveform.len() - 1) as f32 / waveform.len() as f32;
        assert!((points.last().unwrap().x - (rect.left() + last_t * rect.width())).abs() < 1e-5);
    }
}
