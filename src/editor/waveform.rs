use egui::{Color32, Mesh, Painter, Pos2, Rect, Shape, Stroke};

pub const WAVEFORM_LEN: usize = 2049;
pub type WaveformBuffer = [f32; WAVEFORM_LEN];

// pub static ZERO_WAVEFORM: WaveformBuffer = [0.0; WAVEFORM_LEN];

const STROKE_COLOR: Color32 = Color32::from_rgb(0xff, 0xb0, 0x00);
const LINE_WIDTH: f32 = 1.0;

fn fill_top_color() -> Color32 {
    Color32::from_rgba_unmultiplied(0xff, 0xb0, 0x00, 0x47)
}

fn fill_center_color() -> Color32 {
    Color32::from_rgba_unmultiplied(0xff, 0xb0, 0x00, 0x66)
}

#[derive(Clone, Copy, Debug)]
pub struct WaveformOptions {
    pub loop_closed: bool,
    pub normalize: bool,
}

impl Default for WaveformOptions {
    fn default() -> Self {
        Self {
            loop_closed: false,
            normalize: true,
        }
    }
}

/// Linearly interpolate `waveform` at normalized position `t` in `[0, 1]`.
fn sample_at(waveform: &WaveformBuffer, t: f32) -> f32 {
    let last = waveform.len() - 1;
    let pos = t * last as f32;
    let index = pos.floor() as usize;
    let frac = pos - index as f32;
    let from = waveform[index];
    let to = waveform[index.saturating_add(1).min(last)];
    from + (to - from) * frac
}

fn sample_to_y(rect: Rect, sample: f32) -> f32 {
    rect.center().y - sample * rect.height() * 0.5
}

fn build_curve_points(rect: Rect, waveform: &WaveformBuffer) -> Vec<Pos2> {
    let columns = rect.width().ceil().max(2.0) as usize;
    let mut points = Vec::with_capacity(columns);

    for column in 0..columns {
        let t = column as f32 / (columns - 1) as f32;
        let x = rect.left() + t * rect.width();
        let y = sample_to_y(rect, sample_at(waveform, t));
        points.push(Pos2::new(x, y));
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

fn paint_fill(painter: &Painter, rect: Rect, points: &[Pos2], options: WaveformOptions) {
    if points.len() < 2 {
        return;
    }

    let center_y = rect.center().y;
    let mut mesh = Mesh::default();

    let fill_top = fill_top_color();
    let fill_center = fill_center_color();

    let mut add_segment = |a: Pos2, b: Pos2| {
        let ca = Pos2::new(a.x, center_y);
        let cb = Pos2::new(b.x, center_y);

        let i_a = mesh.vertices.len() as u32;
        mesh.colored_vertex(a, fill_top);
        let i_b = mesh.vertices.len() as u32;
        mesh.colored_vertex(b, fill_top);
        let i_cb = mesh.vertices.len() as u32;
        mesh.colored_vertex(cb, fill_center);
        let i_ca = mesh.vertices.len() as u32;
        mesh.colored_vertex(ca, fill_center);

        mesh.add_triangle(i_a, i_b, i_cb);
        mesh.add_triangle(i_a, i_cb, i_ca);
    };

    for window in points.windows(2) {
        add_segment(window[0], window[1]);
    }

    if options.loop_closed {
        add_segment(*points.last().unwrap(), points[0]);
    }

    painter.add(Shape::mesh(mesh));
}

fn paint_stroke(painter: &Painter, points: &[Pos2], options: WaveformOptions) {
    if points.len() < 2 {
        return;
    }

    let stroke = Stroke::new(LINE_WIDTH, STROKE_COLOR);
    painter.line(points.to_vec(), stroke);

    if options.loop_closed {
        painter.line_segment(
            [*points.last().unwrap(), points[0]],
            Stroke::new(LINE_WIDTH, STROKE_COLOR),
        );
    }
}

pub fn paint_waveform(painter: &Painter, rect: Rect, waveform: &WaveformBuffer) {
    paint_waveform_with_options(painter, rect, waveform, WaveformOptions::default());
}

pub fn paint_waveform_with_options(
    painter: &Painter,
    rect: Rect,
    waveform: &WaveformBuffer,
    options: WaveformOptions,
) {
    if !rect.is_positive() {
        return;
    }

    painter.hline(
        rect.x_range(),
        rect.center().y,
        Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 32)),
    );

    let mut points = build_curve_points(rect, waveform);
    if options.normalize {
        normalize_points(rect, &mut points);
    }
    paint_fill(painter, rect, &points, options);
    paint_stroke(painter, &points, options);
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::Vec2;

    #[test]
    fn sample_at_endpoints_match_buffer() {
        let mut waveform = [0.0f32; WAVEFORM_LEN];
        waveform[0] = 1.0;
        waveform[WAVEFORM_LEN - 1] = -1.0;

        assert!((sample_at(&waveform, 0.0) - 1.0).abs() < f32::EPSILON);
        assert!((sample_at(&waveform, 1.0) + 1.0).abs() < f32::EPSILON);
    }

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
}
