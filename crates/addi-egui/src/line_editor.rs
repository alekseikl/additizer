use egui::{
    Color32, CursorIcon, DragValue, Mesh, Painter, PointerButton, Pos2, Rect, Response, Sense,
    Shape, Stroke, Ui, pos2, vec2,
};

use addi_engine::{
    Smp,
    lfo::{LfoLine, LfoPoint, MAX_POWER},
    power_scale,
};

const EDITOR_HEIGHT: f32 = 220.0;
const PAD_Y: f32 = 6.0;
const GRAB_RADIUS: f32 = 12.0;
const POINT_RADIUS: f32 = 4.5;
const POWER_RADIUS: f32 = 3.5;
const MIN_POWER_WIDTH: f32 = 3.0;
const SNAP_X: Smp = 0.02;
const SNAP_Y: Smp = 0.04;
const POWER_MOUSE_MULTIPLIER: Smp = 9.0;

const WAVE_COLOR: Color32 = Color32::from_rgb(0x4a, 0xb0, 0xff);
const FILL_ALPHA: u8 = 0x30;
const POINT_COLOR: Color32 = Color32::from_rgb(0xe8, 0xf4, 0xff);
const POWER_COLOR: Color32 = Color32::from_rgb(0x9a, 0xd0, 0xff);

enum Drag {
    Point(usize),
    Power(usize),
}

enum MenuAction {
    Remove(usize),
    ResetPower(usize),
    FlipHorizontal,
    FlipVertical,
}

pub struct LineEditor {
    grid_x: i32,
    grid_y: i32,
    drag: Option<Drag>,
    last_pointer_y: f32,
    menu_point: Option<usize>,
    menu_power: Option<usize>,
}

impl Default for LineEditor {
    fn default() -> Self {
        Self {
            grid_x: 8,
            grid_y: 1,
            drag: None,
            last_pointer_y: 0.0,
            menu_point: None,
            menu_power: None,
        }
    }
}

impl LineEditor {
    pub fn ui(&mut self, ui: &mut Ui, line: &mut LfoLine, phase: Option<Smp>) -> bool {
        let mut changed = false;
        let width = ui.available_width();
        let (rect, mut response) =
            ui.allocate_exact_size(vec2(width, EDITOR_HEIGHT), Sense::click_and_drag());
        let plot = rect.shrink2(vec2(0.0, PAD_Y));

        if plot.is_positive() {
            changed |= self.interact(ui, &response, plot, line);
            self.paint(ui, plot, rect, line, phase);
            changed |= self.context_menu(&mut response, line);
        }

        ui.add_space(8.0);
        changed |= self.toolbar(ui, line);
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(
                "Double-click to add or remove a point. Drag a midpoint to bend the segment. Right-click to flip.",
            )
            .small()
            .weak(),
        );

        changed
    }

    fn interact(
        &mut self,
        ui: &mut Ui,
        response: &Response,
        plot: Rect,
        line: &mut LfoLine,
    ) -> bool {
        let mut changed = false;
        let modifiers = ui.input(|input| input.modifiers);
        let pointer = response.interact_pointer_pos().or(response.hover_pos());

        if response.secondary_clicked()
            && let Some(pos) = pointer
        {
            let hit = hit_test(plot, line, pos);
            self.menu_point = hit.0;
            self.menu_power = hit.1;
        }

        if response.double_clicked_by(PointerButton::Primary) {
            if let Some(pos) = pointer {
                changed |= double_click(plot, line, pos, modifiers.shift);
            }
            self.drag = None;
        } else if response.drag_started_by(PointerButton::Primary)
            && let Some(pos) = pointer
        {
            let hit = hit_test(plot, line, pos);
            self.drag = hit.0.map(Drag::Point).or_else(|| hit.1.map(Drag::Power));
            self.last_pointer_y = pos.y;
        }

        if response.dragged_by(PointerButton::Primary)
            && let Some(pos) = pointer
        {
            match self.drag {
                Some(Drag::Point(index)) => {
                    let (x, y) = from_pos(plot, pos);
                    move_point(line, index, x, y, !modifiers.alt, self.grid_x, self.grid_y);
                    changed = true;
                }
                Some(Drag::Power(index)) => {
                    let delta = (pos.y - self.last_pointer_y) / plot.height();
                    move_powers(line, index, delta, modifiers.shift, modifiers.alt);
                    self.last_pointer_y = pos.y;
                    changed = true;
                }
                None => {}
            }
        }

        if response.drag_stopped() {
            self.drag = None;
        }

        let hovering = pointer
            .filter(|_| self.drag.is_none() && response.hovered())
            .is_some_and(|pos| {
                let hit = hit_test(plot, line, pos);
                hit.0.is_some() || hit.1.is_some()
            });

        if self.drag.is_some() {
            ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
        } else if hovering {
            ui.ctx().set_cursor_icon(CursorIcon::Grab);
        }

        changed
    }

    fn context_menu(&mut self, response: &mut Response, line: &mut LfoLine) -> bool {
        let menu_point = self.menu_point;
        let menu_power = self.menu_power;
        let mut action = None;

        response.context_menu(|ui| {
            if let Some(index) = menu_point
                && index > 0
                && index + 1 < line.num_points()
                && ui.button("Remove point").clicked()
            {
                action = Some(MenuAction::Remove(index));
                ui.close();
            }

            if let Some(index) = menu_power
                && ui.button("Reset bend").clicked()
            {
                action = Some(MenuAction::ResetPower(index));
                ui.close();
            }

            if ui.button("Flip horizontal").clicked() {
                action = Some(MenuAction::FlipHorizontal);
                ui.close();
            }

            if ui.button("Flip vertical").clicked() {
                action = Some(MenuAction::FlipVertical);
                ui.close();
            }
        });

        match action {
            Some(MenuAction::Remove(index)) => line.remove_point(index),
            Some(MenuAction::ResetPower(index)) => {
                line.set_power(index, 0.0);
                true
            }
            Some(MenuAction::FlipHorizontal) => {
                line.flip_horizontal();
                true
            }
            Some(MenuAction::FlipVertical) => {
                line.flip_vertical();
                true
            }
            None => false,
        }
    }

    fn toolbar(&mut self, ui: &mut Ui, line: &mut LfoLine) -> bool {
        let mut changed = false;

        ui.horizontal_wrapped(|ui| {
            let mut smooth = line.smooth();

            if ui
                .checkbox(&mut smooth, "Smooth")
                .on_hover_text("Ease every segment with a sine curve.")
                .changed()
            {
                line.set_smooth(smooth);
                changed = true;
            }

            ui.separator();
            ui.label("Grid")
                .on_hover_text("Snap points while dragging. Hold Alt to place them freely.");
            ui.add(DragValue::new(&mut self.grid_x).range(1..=32));
            ui.label("×");
            ui.add(DragValue::new(&mut self.grid_y).range(1..=24));

            ui.separator();

            for (label, preset) in PRESETS {
                let shape = preset();
                let selected = *line == shape;

                if ui.selectable_label(selected, *label).clicked() && !selected {
                    *line = shape;
                    changed = true;
                }
            }
        });

        changed
    }

    fn paint(&self, ui: &mut Ui, plot: Rect, frame: Rect, line: &LfoLine, phase: Option<Smp>) {
        let pixels_per_point = ui.pixels_per_point();
        let painter = ui.painter();

        painter.rect_filled(frame, 2.0, ui.visuals().extreme_bg_color);
        painter.rect_stroke(
            frame,
            2.0,
            Stroke::new(1.0, Color32::from_white_alpha(32)),
            egui::StrokeKind::Inside,
        );

        paint_grid(painter, plot, self.grid_x, self.grid_y);

        let curve = curve_points(plot, line, pixels_per_point);

        paint_fill(painter, plot, &curve);

        if curve.len() >= 2 {
            painter.line(curve, Stroke::new(1.5, WAVE_COLOR));
        }

        if let Some(phase) = phase {
            let x = plot.left() + phase.clamp(0.0, 1.0) * plot.width();

            painter.line_segment(
                [pos2(x, plot.top()), pos2(x, plot.bottom())],
                Stroke::new(1.0, with_alpha(WAVE_COLOR, 0x88)),
            );
            painter.circle_filled(to_pos(plot, phase, line.value_at(phase)), 3.5, WAVE_COLOR);
        }

        for index in 0..line.num_points() {
            if !power_active(plot, line, index) {
                continue;
            }

            let (x, y) = power_position(line, index);
            painter.circle_filled(to_pos(plot, x, y), POWER_RADIUS, POWER_COLOR);
        }

        for index in 0..line.num_points() {
            let point = line.point(index);
            painter.circle_filled(to_pos(plot, point.x, point.y), POINT_RADIUS, POINT_COLOR);
        }

        let drag_pos = match self.drag {
            Some(Drag::Point(index)) => {
                let point = line.point(index);
                Some(to_pos(plot, point.x, point.y))
            }
            Some(Drag::Power(index)) => {
                let (x, y) = power_position(line, index);
                Some(to_pos(plot, x, y))
            }
            None => None,
        };

        if let Some(pos) = drag_pos {
            painter.circle_stroke(pos, GRAB_RADIUS, Stroke::new(1.0, WAVE_COLOR));
        }
    }
}

type Preset = (&'static str, fn() -> LfoLine);

const PRESETS: &[Preset] = &[
    ("Triangle", LfoLine::triangle),
    ("Square", LfoLine::square),
    ("Sine", LfoLine::sine),
    ("Saw up", LfoLine::saw_up),
    ("Saw down", LfoLine::saw_down),
    ("Ramp", LfoLine::linear),
];

fn double_click(plot: Rect, line: &mut LfoLine, pos: Pos2, shift: bool) -> bool {
    let (point, power) = hit_test(plot, line, pos);

    if let Some(index) = point {
        return line.remove_point(index);
    }

    if let Some(index) = power {
        if shift {
            line.reset_powers();
        } else {
            line.set_power(index, 0.0);
        }
        return true;
    }

    let (x, y) = from_pos(plot, pos);
    let mut index = 0;
    let n = line.num_points();

    while index < n && line.point(index).x <= x {
        index += 1;
    }

    line.add_point(index, LfoPoint { x, y })
}

fn move_point(
    line: &mut LfoLine,
    index: usize,
    x: Smp,
    y: Smp,
    snap: bool,
    grid_x: i32,
    grid_y: i32,
) {
    let n = line.num_points();

    if index >= n {
        return;
    }

    let (min_x, max_x) = if index == 0 {
        (0.0, 0.0)
    } else if index + 1 == n {
        (1.0, 1.0)
    } else {
        (line.point(index - 1).x, line.point(index + 1).x)
    };

    let mut x = x.clamp(min_x, max_x);
    let mut y = y.clamp(0.0, 1.0);

    if snap && grid_x > 1 {
        let snapped = (x * grid_x as Smp).round() / grid_x as Smp;

        if (snapped - x).abs() < SNAP_X {
            x = snapped.clamp(min_x, max_x);
        }
    }

    if snap && grid_y > 1 {
        let snapped = (y * grid_y as Smp).round() / grid_y as Smp;

        if (snapped - y).abs() < SNAP_Y {
            y = snapped.clamp(0.0, 1.0);
        }
    }

    let linked = line.point(0).y == line.point(n - 1).y;

    if linked && index == 0 {
        let end = line.point(n - 1);
        line.set_point(n - 1, LfoPoint { x: end.x, y });
    } else if linked && index + 1 == n {
        let start = line.point(0);
        line.set_point(0, LfoPoint { x: start.x, y });
    }

    line.set_point(index, LfoPoint { x, y });
}

fn move_powers(line: &mut LfoLine, index: usize, delta_change: Smp, all: bool, alternate: bool) {
    let n = line.num_points();

    if index >= n || n < 2 {
        return;
    }

    let (start, end) = if all { (0, n - 2) } else { (index, index) };
    let descending = line.point(index).y > segment_end_y(line, index);
    let alternate_mult = if !alternate && descending { -1.0 } else { 1.0 };

    for i in start..=end {
        let from_y = line.point(i).y;
        let to_y = segment_end_y(line, i);

        if from_y == to_y {
            continue;
        }

        let mut delta = delta_change * alternate_mult;

        if from_y > to_y && alternate {
            delta = -delta;
        }

        line.set_power(
            i,
            (line.power(i) + delta * POWER_MOUSE_MULTIPLIER).clamp(-MAX_POWER, MAX_POWER),
        );
    }
}

fn segment_end_y(line: &LfoLine, index: usize) -> Smp {
    let n = line.num_points();

    if index + 1 < n {
        line.point(index + 1).y
    } else {
        line.point(0).y
    }
}

fn hit_test(plot: Rect, line: &LfoLine, pos: Pos2) -> (Option<usize>, Option<usize>) {
    let grab_sq = GRAB_RADIUS * GRAB_RADIUS;
    let mut best_point = None;
    let mut best_distance = grab_sq;

    for index in 0..line.num_points() {
        let point = line.point(index);
        let distance = pos.distance_sq(to_pos(plot, point.x, point.y));

        if distance < best_distance {
            best_distance = distance;
            best_point = Some(index);
        }
    }

    if best_point.is_some() {
        return (best_point, None);
    }

    let mut best_power = None;
    best_distance = grab_sq;

    for index in 0..line.num_points() {
        if !power_active(plot, line, index) {
            continue;
        }

        let (x, y) = power_position(line, index);
        let distance = pos.distance_sq(to_pos(plot, x, y));

        if distance < best_distance {
            best_distance = distance;
            best_power = Some(index);
        }
    }

    (None, best_power)
}

fn power_active(plot: Rect, line: &LfoLine, index: usize) -> bool {
    let from = line.point(index);
    let (to_x, to_y) = segment_end(line, index);
    let dx = to_x - from.x;
    let dy = to_y - from.y;

    plot.width() * dx >= MIN_POWER_WIDTH && dy != 0.0
}

fn segment_end(line: &LfoLine, index: usize) -> (Smp, Smp) {
    let n = line.num_points();

    if index + 1 < n {
        let point = line.point(index + 1);
        (point.x, point.y)
    } else {
        let point = line.point(0);
        (point.x + 1.0, point.y)
    }
}

fn power_position(line: &LfoLine, index: usize) -> (Smp, Smp) {
    let from = line.point(index);
    let (to_x, to_y) = segment_end(line, index);
    let mut x = (from.x + to_x) * 0.5;

    if x >= 1.0 {
        x -= 1.0;
    }

    let t = power_scale(0.5, line.power(index));
    let y = from.y + t * (to_y - from.y);

    (x, y)
}

/// Each segment is sampled once per horizontal pixel, and every sample is kept.
/// A segment that lands on one pixel column keeps both endpoints, so a vertical
/// jump stays a vertical line.
fn curve_points(plot: Rect, line: &LfoLine, pixels_per_point: f32) -> Vec<Pos2> {
    if plot.width() <= 0.0 || pixels_per_point <= 0.0 || line.num_points() < 2 {
        return Vec::new();
    }

    let mut points = Vec::new();

    for index in 0..line.num_points() - 1 {
        append_segment_points(&mut points, plot, line, index, pixels_per_point);
    }

    points
}

fn append_segment_points(
    points: &mut Vec<Pos2>,
    plot: Rect,
    line: &LfoLine,
    index: usize,
    pixels_per_point: f32,
) {
    let start = to_pos(plot, line.point(index).x, line.point(index).y);
    let end = to_pos(plot, line.point(index + 1).x, line.point(index + 1).y);
    let start_px = (start.x * pixels_per_point).round() as i32;
    let end_px = (end.x * pixels_per_point).round() as i32;

    push_point(points, start);

    if end_px > start_px {
        for px in start_px + 1..end_px {
            let x = px as f32 / pixels_per_point;
            let phase = ((x - plot.left()) / plot.width()).clamp(0.0, 1.0);
            let y = line.value_between(phase, index, index + 1);

            push_point(points, pos2(x, to_pos(plot, phase, y).y));
        }
    }

    push_point(points, end);
}

fn push_point(points: &mut Vec<Pos2>, point: Pos2) {
    if points.last() != Some(&point) {
        points.push(point);
    }
}

fn from_pos(plot: Rect, pos: Pos2) -> (Smp, Smp) {
    let x = ((pos.x - plot.left()) / plot.width()).clamp(0.0, 1.0);
    let y = ((plot.bottom() - pos.y) / plot.height()).clamp(0.0, 1.0);

    (x, y)
}

fn to_pos(plot: Rect, x: Smp, y: Smp) -> Pos2 {
    pos2(
        plot.left() + x.clamp(0.0, 1.0) * plot.width(),
        plot.bottom() - y.clamp(0.0, 1.0) * plot.height(),
    )
}

fn paint_grid(painter: &Painter, plot: Rect, grid_x: i32, grid_y: i32) {
    let stroke = Stroke::new(1.0, Color32::from_white_alpha(28));

    if grid_x > 1 {
        for i in 1..grid_x {
            let x = plot.left() + plot.width() * (i as f32 / grid_x as f32);
            painter.line_segment([pos2(x, plot.top()), pos2(x, plot.bottom())], stroke);
        }
    }

    if grid_y > 1 {
        for i in 1..grid_y {
            let y = plot.bottom() - plot.height() * (i as f32 / grid_y as f32);
            painter.line_segment([pos2(plot.left(), y), pos2(plot.right(), y)], stroke);
        }
    }
}

fn paint_fill(painter: &Painter, plot: Rect, points: &[Pos2]) {
    if points.len() < 2 {
        return;
    }

    let mut mesh = Mesh::default();
    let color = with_alpha(WAVE_COLOR, FILL_ALPHA);

    for window in points.windows(2) {
        let a = window[0];
        let b = window[1];
        let index = mesh.vertices.len() as u32;

        mesh.colored_vertex(a, color);
        mesh.colored_vertex(b, color);
        mesh.colored_vertex(pos2(b.x, plot.bottom()), color);
        mesh.colored_vertex(pos2(a.x, plot.bottom()), color);
        mesh.add_triangle(index, index + 1, index + 2);
        mesh.add_triangle(index, index + 2, index + 3);
    }

    painter.add(Shape::mesh(mesh));
}

fn with_alpha(color: Color32, alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha)
}

#[cfg(test)]
mod tests {
    use egui::{Rect, pos2};

    use addi_engine::lfo::LfoLine;

    use super::curve_points;

    fn plot() -> Rect {
        Rect::from_min_max(pos2(0.0, 0.0), pos2(100.0, 100.0))
    }

    #[test]
    fn linear_segment_keeps_a_point_per_pixel() {
        let points = curve_points(plot(), &LfoLine::linear(), 1.0);

        assert_eq!(points.len(), 101);
        assert_eq!(points[0], pos2(0.0, 100.0));
        assert_eq!(points[40], pos2(40.0, 60.0));
        assert_eq!(points[100], pos2(100.0, 0.0));
    }

    #[test]
    fn square_keeps_vertical_edges() {
        let points = curve_points(plot(), &LfoLine::square(), 1.0);

        assert_eq!(&points[..2], &[pos2(0.0, 100.0), pos2(0.0, 0.0)]);
        assert!(points.contains(&pos2(50.0, 0.0)));
        assert!(points.contains(&pos2(50.0, 100.0)));
        assert_eq!(points[points.len() - 1], pos2(100.0, 100.0));
    }

    #[test]
    fn smooth_curve_keeps_intermediate_pixels() {
        let points = curve_points(plot(), &LfoLine::sine(), 1.0);

        assert!(points.len() > 3);
        assert_eq!(points[0], pos2(0.0, 100.0));
        assert!(points.contains(&pos2(50.0, 0.0)));
        assert_eq!(points[points.len() - 1], pos2(100.0, 100.0));
    }
}
