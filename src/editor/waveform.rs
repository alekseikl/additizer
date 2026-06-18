use egui::{
    Color32, Mesh, Painter, Pos2, Rect, Response, Sense, Shape, Stroke, Ui, Vec2,
};

pub const WAVEFORM_LEN: usize = 2048;
pub type WaveformBuffer = [f32; WAVEFORM_LEN];

// Vital default skin (default.vitalskin).
const BG_COLOR: Color32 = Color32::from_rgb(0x1d, 0x21, 0x25);
const STROKE_COLOR: Color32 = Color32::from_rgb(0xaa, 0x88, 0xff);
const LINE_WIDTH: f32 = 2.0;

fn fill_top_color() -> Color32 {
    Color32::from_rgba_unmultiplied(0x9f, 0x88, 0xff, 0x47)
}

fn fill_center_color() -> Color32 {
    Color32::from_rgba_unmultiplied(0x9f, 0x88, 0xff, 0x66)
}

/// Display options aligned with Vital's `WaveSourceEditor` / oscilloscope widgets.
#[derive(Clone, Copy, Debug)]
pub struct WaveformOptions {
    /// Connect the last sample back to the first, as Vital does for wavetable frames.
    pub loop_closed: bool,
    /// Clamp samples to ±1.0 before mapping (Vital's wavetable editor range).
    pub clamp_amplitude: bool,
}

impl Default for WaveformOptions {
    fn default() -> Self {
        Self {
            loop_closed: true,
            clamp_amplitude: true,
        }
    }
}

fn clamp_sample(sample: f32, clamp_amplitude: bool) -> f32 {
    if clamp_amplitude {
        sample.clamp(-1.0, 1.0)
    } else {
        sample
    }
}

/// Linearly interpolate `waveform` at normalized position `t` in `[0, 1]`.
fn sample_at(waveform: &WaveformBuffer, t: f32, clamp_amplitude: bool) -> f32 {
    let last = waveform.len() - 1;
    let pos = t * last as f32;
    let index = pos.floor() as usize;
    let frac = pos - index as f32;
    let from = clamp_sample(waveform[index], clamp_amplitude);
    let to = clamp_sample(waveform[index.saturating_add(1).min(last)], clamp_amplitude);
    from + (to - from) * frac
}

fn sample_to_y(rect: Rect, sample: f32) -> f32 {
    rect.center().y - sample * rect.height() * 0.5
}

fn build_curve_points(rect: Rect, waveform: &WaveformBuffer, options: WaveformOptions) -> Vec<Pos2> {
    let columns = rect.width().ceil().max(2.0) as usize;
    let mut points = Vec::with_capacity(columns);

    for column in 0..columns {
        let t = column as f32 / (columns - 1) as f32;
        let x = rect.left() + t * rect.width();
        let y = sample_to_y(rect, sample_at(waveform, t, options.clamp_amplitude));
        points.push(Pos2::new(x, y));
    }

    points
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

/// Paints a single-cycle waveform in Vital's wavetable style:
/// fixed ±1 amplitude, linear interpolation, center-line fill, and lavender stroke.
pub fn paint_waveform(painter: &Painter, rect: Rect, waveform: &WaveformBuffer) {
    paint_waveform_with_options(painter, rect, waveform, WaveformOptions::default());
}

/// Paints a waveform with explicit Vital-style options.
pub fn paint_waveform_with_options(
    painter: &Painter,
    rect: Rect,
    waveform: &WaveformBuffer,
    options: WaveformOptions,
) {
    if !rect.is_positive() {
        return;
    }

    painter.rect_filled(rect, 0.0, BG_COLOR);
    painter.hline(
        rect.x_range(),
        rect.center().y,
        Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 32)),
    );

    let points = build_curve_points(rect, waveform, options);
    paint_fill(painter, rect, &points, options);
    paint_stroke(painter, &points, options);
}

/// Allocates space in the layout and paints `waveform` into it.
pub fn draw_waveform(ui: &mut Ui, waveform: &WaveformBuffer, desired_size: Vec2) -> Response {
    draw_waveform_with_options(ui, waveform, desired_size, WaveformOptions::default())
}

/// Allocates space in the layout and paints `waveform` with explicit options.
pub fn draw_waveform_with_options(
    ui: &mut Ui,
    waveform: &WaveformBuffer,
    desired_size: Vec2,
    options: WaveformOptions,
) -> Response {
    let response = ui.allocate_response(desired_size, Sense::hover());

    if ui.is_rect_visible(response.rect) {
        paint_waveform_with_options(ui.painter(), response.rect, waveform, options);
    }

    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_at_endpoints_match_buffer() {
        let mut waveform = [0.0f32; WAVEFORM_LEN];
        waveform[0] = 1.0;
        waveform[WAVEFORM_LEN - 1] = -1.0;

        let options = WaveformOptions::default();
        assert!((sample_at(&waveform, 0.0, options.clamp_amplitude) - 1.0).abs() < f32::EPSILON);
        assert!((sample_at(&waveform, 1.0, options.clamp_amplitude) + 1.0).abs() < f32::EPSILON);
    }
}
