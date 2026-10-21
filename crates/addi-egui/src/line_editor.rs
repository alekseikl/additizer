use egui::{
    Color32, CursorIcon, DragValue, Mesh, Painter, PointerButton, Pos2, Rect, Response, Sense,
    Shape, Stroke, Ui, pos2, vec2,
};

use addi_dsp::{
    power_scale,
    segments::{CurvePoint, MAX_POWER, Segment, Segments},
    utils::sanitize,
};
use addi_engine::Smp;

use crate::segment_presets;

const EDITOR_HEIGHT: f32 = 220.0;
const GRAB_RADIUS: f32 = 12.0;
const POINT_RADIUS: f32 = 4.5;
const POWER_RADIUS: f32 = 3.5;
const MIN_POWER_WIDTH: f32 = 3.0;
const SNAP_X: Smp = 0.02;
const SNAP_Y: Smp = 0.04;
const POWER_MOUSE_MULTIPLIER: Smp = 9.0;

const WAVE_COLOR: Color32 = Color32::from_rgb(0x4a, 0xb0, 0xff);
const FILL_TOP_ALPHA: u8 = 0x47;
const FILL_BOTTOM_ALPHA: u8 = 0x28;
const GRID_COLOR: Color32 = Color32::from_rgba_unmultiplied_const(255, 255, 255, 28);
const POINT_COLOR: Color32 = Color32::from_rgb(0xe8, 0xf4, 0xff);
const POWER_COLOR: Color32 = Color32::from_rgb(0x9a, 0xd0, 0xff);

type Preset = (&'static str, fn() -> Segments);

const PRESETS: &[Preset] = &[
    ("Triangle", segment_presets::triangle),
    ("Square", segment_presets::square),
    ("Sine", segment_presets::sine),
    ("Saw up", segment_presets::saw_up),
    ("Saw down", segment_presets::saw_down),
];

#[derive(Clone, Copy)]
enum Drag {
    Point(usize),
    Power(usize),
}

enum MenuAction {
    Remove(usize),
    ResetPower(usize),
    SetSmooth(usize, bool),
    FlipHorizontal,
    FlipVertical,
}

pub struct SegmentsEditor {
    grid_x: i32,
    grid_y: i32,
    drag: Option<Drag>,
    hover: Option<Drag>,
    last_pointer_y: f32,
    menu_point: Option<usize>,
    menu_power: Option<usize>,
    curve: Vec<CurvePoint>,
    scratch: Vec<Smp>,
    points: Vec<Pos2>,
}

impl Default for SegmentsEditor {
    fn default() -> Self {
        Self {
            grid_x: 8,
            grid_y: 1,
            drag: None,
            hover: None,
            last_pointer_y: 0.0,
            menu_point: None,
            menu_power: None,
            curve: Vec::new(),
            scratch: Vec::new(),
            points: Vec::new(),
        }
    }
}

impl SegmentsEditor {
    pub fn ui(&mut self, ui: &mut Ui, segments: &mut Segments, phase: Option<Smp>) -> bool {
        let mut changed = false;
        let width = ui.available_width();
        let (rect, mut response) =
            ui.allocate_exact_size(vec2(width, EDITOR_HEIGHT), Sense::click_and_drag());

        if rect.is_positive() {
            changed |= self.interact(ui, &response, rect, segments);
            self.paint(ui, rect, segments, phase);
            changed |= self.context_menu(&mut response, segments);
        }

        ui.add_space(8.0);
        changed |= self.toolbar(ui, segments);
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
        segments: &mut Segments,
    ) -> bool {
        let mut changed = false;
        let modifiers = ui.input(|input| input.modifiers);
        let pointer = response.interact_pointer_pos().or(response.hover_pos());

        if response.secondary_clicked()
            && let Some(pos) = pointer
        {
            let hit = Self::hit_test(plot, segments, pos);
            self.menu_point = hit.0;
            self.menu_power = hit.1;
        }

        if response.double_clicked_by(PointerButton::Primary) {
            if let Some(pos) = pointer {
                changed |= Self::double_click(plot, segments, pos, modifiers.shift);
            }
            self.drag = None;
        } else if response.drag_started_by(PointerButton::Primary)
            && let Some(pos) = pointer
        {
            let hit = Self::hit_test(plot, segments, pos);
            self.drag = hit.0.map(Drag::Point).or_else(|| hit.1.map(Drag::Power));
            self.last_pointer_y = pos.y;
        }

        if response.dragged_by(PointerButton::Primary)
            && let Some(pos) = pointer
        {
            match self.drag {
                Some(Drag::Point(index)) => {
                    let (x, y) = Self::from_pos(plot, pos);
                    Self::move_point(
                        segments,
                        index,
                        x,
                        y,
                        !modifiers.alt,
                        self.grid_x,
                        self.grid_y,
                    );
                    changed = true;
                }
                Some(Drag::Power(index)) => {
                    let delta = (pos.y - self.last_pointer_y) / plot.height();
                    Self::move_powers(segments, index, delta, modifiers.shift, modifiers.alt);
                    self.last_pointer_y = pos.y;
                    changed = true;
                }
                None => {}
            }
        }

        if response.drag_stopped() {
            self.drag = None;
        }

        self.hover = if self.drag.is_none() && response.hovered() {
            pointer.and_then(|pos| {
                let hit = Self::hit_test(plot, segments, pos);
                hit.0.map(Drag::Point).or_else(|| hit.1.map(Drag::Power))
            })
        } else {
            None
        };

        if self.drag.is_some() {
            ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
        } else if self.hover.is_some() {
            ui.ctx().set_cursor_icon(CursorIcon::Grab);
        }

        changed
    }

    fn context_menu(&mut self, response: &mut Response, segments: &mut Segments) -> bool {
        let menu_point = self.menu_point;
        let menu_power = self.menu_power;
        let mut action = None;

        response.context_menu(|ui| {
            if let Some(index) = menu_point.or(menu_power)
                && index < segments.num_segments()
            {
                let smooth = segments.segment(index).smooth;
                let label = if smooth {
                    "Sharpen segment"
                } else {
                    "Smooth segment"
                };

                if ui
                    .button(label)
                    .on_hover_text("Ease this segment with a sine curve.")
                    .clicked()
                {
                    action = Some(MenuAction::SetSmooth(index, !smooth));
                    ui.close();
                }
            }

            if let Some(index) = menu_point
                && index > 0
                && index + 1 < segments.num_segments()
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

            if menu_point.is_none() && menu_power.is_none() {
                if ui.button("Flip horizontal").clicked() {
                    action = Some(MenuAction::FlipHorizontal);
                    ui.close();
                }

                if ui.button("Flip vertical").clicked() {
                    action = Some(MenuAction::FlipVertical);
                    ui.close();
                }
            }
        });

        match action {
            Some(MenuAction::Remove(index)) => segments.remove_segment(index),
            Some(MenuAction::ResetPower(index)) => {
                segments.set_power(index, 0.0);
                true
            }
            Some(MenuAction::SetSmooth(index, smooth)) => {
                segments.set_smooth(index, smooth);
                true
            }
            Some(MenuAction::FlipHorizontal) => {
                segments.flip_horizontal();
                true
            }
            Some(MenuAction::FlipVertical) => {
                segments.flip_vertical();
                true
            }
            None => false,
        }
    }

    fn toolbar(&mut self, ui: &mut Ui, segments: &mut Segments) -> bool {
        let mut changed = false;

        ui.horizontal_wrapped(|ui| {
            ui.label("Grid")
                .on_hover_text("Snap points while dragging. Hold Alt to place them freely.");
            ui.add(DragValue::new(&mut self.grid_x).range(1..=32));
            ui.label("×");
            ui.add(DragValue::new(&mut self.grid_y).range(1..=24));

            ui.separator();

            for (label, preset) in PRESETS {
                if ui.button(*label).clicked() {
                    *segments = preset();
                    changed = true;
                }
            }
        });

        changed
    }

    fn paint(&mut self, ui: &mut Ui, plot: Rect, segments: &Segments, phase: Option<Smp>) {
        let pixels_per_point = ui.pixels_per_point();
        let painter = ui.painter();
        let background = ui.visuals().extreme_bg_color;

        painter.rect_filled(plot, 2.0, background);
        painter.rect_stroke(
            plot,
            2.0,
            Stroke::new(1.0, Color32::from_white_alpha(32)),
            egui::StrokeKind::Inside,
        );

        self.curve_points(plot, segments, pixels_per_point);

        Self::paint_fill(painter, plot, &self.points, background);
        Self::paint_grid(painter, plot, self.grid_x, self.grid_y);

        if self.points.len() >= 2 {
            painter.line(self.points.clone(), Stroke::new(1.5, WAVE_COLOR));
        }

        for index in 0..segments.num_segments() {
            if !Self::power_active(plot, segments, index) {
                continue;
            }

            let (x, y) = Self::power_position(segments, index);
            painter.circle_filled(Self::to_pos(plot, x, y), POWER_RADIUS, POWER_COLOR);
        }

        for index in 0..segments.num_segments() {
            let point = segments.segment(index);
            painter.circle_filled(
                Self::to_pos(plot, point.x, point.y),
                POINT_RADIUS,
                POINT_COLOR,
            );
        }

        if let Some(phase) = phase {
            painter.circle_filled(
                Self::curve_pos(plot, segments, phase),
                POINT_RADIUS,
                WAVE_COLOR,
            );
        }

        let grab_stroke = Stroke::new(1.0, Color32::WHITE);

        if let Some(drag) = self.drag {
            let pos = Self::target_pos(drag, plot, segments);
            painter.circle_filled(pos, GRAB_RADIUS, Color32::from_white_alpha(0x40));
            painter.circle_stroke(pos, GRAB_RADIUS, grab_stroke);
        } else if let Some(hover) = self.hover {
            painter.circle_stroke(
                Self::target_pos(hover, plot, segments),
                GRAB_RADIUS,
                grab_stroke,
            );
        }
    }

    fn double_click(plot: Rect, segments: &mut Segments, pos: Pos2, shift: bool) -> bool {
        let (point, power) = Self::hit_test(plot, segments, pos);

        if let Some(index) = point {
            return segments.remove_segment(index);
        }

        if let Some(index) = power {
            if shift {
                segments.reset_powers();
            } else {
                segments.set_power(index, 0.0);
            }
            return true;
        }

        let (x, y) = Self::from_pos(plot, pos);
        let mut index = 0;
        let n = segments.num_segments();

        while index < n && segments.segment(index).x <= x {
            index += 1;
        }

        let smooth = if index == 0 {
            segments.segment(0).smooth
        } else {
            segments.segment(index - 1).smooth
        };

        segments.add_segment(
            index,
            Segment {
                x,
                y,
                smooth,
                ..Segment::default()
            },
        )
    }

    fn move_point(
        segments: &mut Segments,
        index: usize,
        x: Smp,
        y: Smp,
        snap: bool,
        grid_x: i32,
        grid_y: i32,
    ) {
        let n = segments.num_segments();

        if index >= n {
            return;
        }

        let (min_x, max_x) = if index == 0 {
            (0.0, 0.0)
        } else if index + 1 == n {
            (1.0, 1.0)
        } else {
            (segments.segment(index - 1).x, segments.segment(index + 1).x)
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

        let linked = segments.segment(0).y == segments.segment(n - 1).y;

        if linked && index == 0 {
            let end = segments.segment(n - 1);
            segments.set_segment(n - 1, Segment { y, ..end });
        } else if linked && index + 1 == n {
            let start = segments.segment(0);
            segments.set_segment(0, Segment { y, ..start });
        }

        let current = segments.segment(index);
        segments.set_segment(index, Segment { x, y, ..current });
    }

    fn move_powers(
        segments: &mut Segments,
        index: usize,
        delta_change: Smp,
        all: bool,
        alternate: bool,
    ) {
        let n = segments.num_segments();

        if index >= n || n < 2 {
            return;
        }

        let (start, end) = if all { (0, n - 2) } else { (index, index) };
        let descending = segments.segment(index).y > Self::segment_end_y(segments, index);
        let alternate_mult = if !alternate && descending { -1.0 } else { 1.0 };

        for i in start..=end {
            let from_y = segments.segment(i).y;
            let to_y = Self::segment_end_y(segments, i);

            if from_y == to_y {
                continue;
            }

            let mut delta = delta_change * alternate_mult;

            if from_y > to_y && alternate {
                delta = -delta;
            }

            segments.set_power(
                i,
                (segments.power(i) + delta * POWER_MOUSE_MULTIPLIER).clamp(-MAX_POWER, MAX_POWER),
            );
        }
    }

    fn segment_end_y(segments: &Segments, index: usize) -> Smp {
        let n = segments.num_segments();

        if index + 1 < n {
            segments.segment(index + 1).y
        } else {
            segments.segment(0).y
        }
    }

    fn hit_test(plot: Rect, segments: &Segments, pos: Pos2) -> (Option<usize>, Option<usize>) {
        let grab_sq = GRAB_RADIUS * GRAB_RADIUS;
        let mut best_point = None;
        let mut best_distance = grab_sq;

        for index in 0..segments.num_segments() {
            let point = segments.segment(index);
            let distance = pos.distance_sq(Self::to_pos(plot, point.x, point.y));

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

        for index in 0..segments.num_segments() {
            if !Self::power_active(plot, segments, index) {
                continue;
            }

            let (x, y) = Self::power_position(segments, index);
            let distance = pos.distance_sq(Self::to_pos(plot, x, y));

            if distance < best_distance {
                best_distance = distance;
                best_power = Some(index);
            }
        }

        (None, best_power)
    }

    fn power_active(plot: Rect, segments: &Segments, index: usize) -> bool {
        let from = segments.segment(index);
        let (to_x, to_y) = Self::segment_end(segments, index);
        let dx = to_x - from.x;
        let dy = to_y - from.y;

        plot.width() * dx >= MIN_POWER_WIDTH && dy != 0.0
    }

    fn segment_end(segments: &Segments, index: usize) -> (Smp, Smp) {
        let n = segments.num_segments();

        if index + 1 < n {
            let point = segments.segment(index + 1);
            (point.x, point.y)
        } else {
            let point = segments.segment(0);
            (point.x + 1.0, point.y)
        }
    }

    fn power_position(segments: &Segments, index: usize) -> (Smp, Smp) {
        let from = segments.segment(index);
        let (to_x, to_y) = Self::segment_end(segments, index);
        let mut x = (from.x + to_x) * 0.5;

        if x >= 1.0 {
            x -= 1.0;
        }

        let t = power_scale(0.5, segments.power(index));
        let y = from.y + t * (to_y - from.y);

        (x, y)
    }

    /// Closed polyline for one sample per horizontal pixel, with each breakpoint kept.
    fn curve_points(&mut self, plot: Rect, segments: &Segments, pixels_per_point: f32) {
        if plot.width() <= 0.0 || pixels_per_point <= 0.0 || segments.num_segments() < 2 {
            self.points.clear();
            return;
        }

        let steps = (plot.width() * pixels_per_point).round().max(1.0) as usize;

        segments.render_points(steps, &mut self.curve, &mut self.scratch);
        self.points.clear();
        self.points.reserve(self.curve.len());

        for point in &self.curve {
            self.points.push(Self::to_pos(plot, point.x, point.y));
        }
    }

    /// Screen position of `phase` on `segments`.
    fn curve_pos(plot: Rect, segments: &Segments, phase: Smp) -> Pos2 {
        let phase = sanitize(phase, 0.0, 1.0);

        Self::to_pos(plot, phase, segments.value_at(phase))
    }

    fn from_pos(plot: Rect, pos: Pos2) -> (Smp, Smp) {
        let x = ((pos.x - plot.left()) / plot.width()).clamp(0.0, 1.0);
        let y = ((plot.bottom() - pos.y) / plot.height()).clamp(0.0, 1.0);

        (x, y)
    }

    fn target_pos(target: Drag, plot: Rect, segments: &Segments) -> Pos2 {
        match target {
            Drag::Point(index) => {
                let point = segments.segment(index);
                Self::to_pos(plot, point.x, point.y)
            }
            Drag::Power(index) => {
                let (x, y) = Self::power_position(segments, index);
                Self::to_pos(plot, x, y)
            }
        }
    }

    fn to_pos(plot: Rect, x: Smp, y: Smp) -> Pos2 {
        pos2(
            plot.left() + x.clamp(0.0, 1.0) * plot.width(),
            plot.bottom() - y.clamp(0.0, 1.0) * plot.height(),
        )
    }

    fn paint_grid(painter: &Painter, plot: Rect, grid_x: i32, grid_y: i32) {
        let stroke = Stroke::new(1.0, GRID_COLOR);

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

    /// One gradient rectangle, then an opaque cover above the curve.
    /// A per-column mesh creases the gradient: vertical lines from premultiplied
    /// vertex colors, horizontal lines from sampling a gradient texture.
    fn paint_fill(painter: &Painter, plot: Rect, points: &[Pos2], background: Color32) {
        if plot.height() <= 0.0 || plot.width() <= 0.0 {
            return;
        }

        let mut gradient = Mesh::default();
        let top = Self::with_alpha(WAVE_COLOR, FILL_TOP_ALPHA);
        let bottom = Self::with_alpha(WAVE_COLOR, FILL_BOTTOM_ALPHA);

        gradient.colored_vertex(plot.left_top(), top);
        gradient.colored_vertex(plot.right_top(), top);
        gradient.colored_vertex(plot.right_bottom(), bottom);
        gradient.colored_vertex(plot.left_bottom(), bottom);
        gradient.add_triangle(0, 1, 2);
        gradient.add_triangle(0, 2, 3);
        painter.add(Shape::mesh(gradient));

        if points.len() < 2 {
            return;
        }

        let cover = Color32::from_rgb(background.r(), background.g(), background.b());
        let mut mask = Mesh::default();

        for window in points.windows(2) {
            let a = window[0];
            let b = window[1];

            if b.x <= a.x {
                continue;
            }

            let index = mask.vertices.len() as u32;

            mask.colored_vertex(pos2(a.x, plot.top()), cover);
            mask.colored_vertex(pos2(b.x, plot.top()), cover);
            mask.colored_vertex(b, cover);
            mask.colored_vertex(a, cover);
            mask.add_triangle(index, index + 1, index + 2);
            mask.add_triangle(index, index + 2, index + 3);
        }

        if !mask.is_empty() {
            painter.add(Shape::mesh(mask));
        }
    }

    fn with_alpha(color: Color32, alpha: u8) -> Color32 {
        Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha)
    }
}

#[cfg(test)]
mod tests;
