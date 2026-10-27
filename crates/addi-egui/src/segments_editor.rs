use egui::{
    Color32, CursorIcon, DragValue, FontFamily, FontId, Id, LayerId, Mesh, Order, Painter,
    PointerButton, Pos2, Rect, Response, Sense, Shape, Stroke, StrokeKind, Ui, Vec2, ecolor::Hsva,
    pos2, vec2,
};

use addi_dsp::{
    power_scale,
    segments::{MAX_POWER, SegPoint, Segments},
    utils::sanitize,
};
use addi_engine::{Smp, StereoSample};

use crate::{circle_grab::CircleGrab, segment_presets, units::Units, utils::hsva};

const EDITOR_HEIGHT: f32 = 220.0;
const GRAB_RADIUS: f32 = 8.0;
const POWER_GRAB_RADIUS: f32 = 6.0;
const POINT_RADIUS: f32 = 4.0;
const PLAYHEAD_RADIUS: f32 = 6.0;
/// Yellow. Saturation and value are taken from the left playhead.
const RIGHT_PLAYHEAD_HUE: f32 = 1.0 / 6.0;
const POWER_RADIUS: f32 = 3.0;
const MIN_POWER_WIDTH: f32 = 3.0;
const SNAP_X: Smp = 0.02;
const SNAP_Y: Smp = 0.04;
/// One editor-height of vertical travel. Same scale as Vital's line editor.
const POWER_MOUSE_MULTIPLIER: Smp = 9.0;

const WAVE_COLOR: Color32 = Color32::from_rgb(0x4a, 0xb0, 0xff);
const LOOP_COLOR: Color32 = Color32::from_rgb(0xf0, 0xc0, 0x4a);
const LOOP_FILL_COLOR: Hsva = hsva(0.0, 0.9092, 1.0, 0.25);
const RELEASE_COLOR: Color32 = Color32::from_rgb(0xff, 0x7a, 0x59);
const FILL_TOP_ALPHA: u8 = 0x47;
const FILL_BOTTOM_ALPHA: u8 = 0x28;
const GRID_COLOR: Color32 = Color32::from_rgba_unmultiplied_const(255, 255, 255, 28);
const POINT_COLOR: Color32 = Color32::from_rgb(0xe8, 0xf4, 0xff);
const POWER_COLOR: Color32 = Color32::from_rgb(0x9a, 0xd0, 0xff);
const LABEL_BG_COLOR: Hsva = hsva(0.115, 0.05, 0.01, 1.0);
const LABEL_BORDER_COLOR: Hsva = hsva(0.115, 0.05, 0.2, 1.0);
const LABEL_TEXT_COLOR: Hsva = hsva(0.115, 0.05, 0.5, 1.0);
const LABEL_PADDING: f32 = 6.0;
const LABEL_POINTER_GAP: f32 = 4.0;
const LABEL_CORNER_RADIUS: f32 = 2.0;

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
    SetLoop(usize),
    ClearLoop,
    SetRelease(usize),
    ClearRelease,
}

struct RenderedCurve {
    revision: u64,
    size: Vec2,
}

pub struct SegmentsEditor {
    grid_x: i32,
    grid_y: i32,
    drag: Option<Drag>,
    grab: Option<CircleGrab>,
    /// Curve-space pointer y at the previous bend sample.
    power_y: Option<Smp>,
    hover: Option<Drag>,
    menu_point: Option<usize>,
    menu_power: Option<usize>,
    curve: Vec<[Smp; 2]>,
    scratch: Vec<Smp>,
    points: Vec<Pos2>,
    rendered: Option<RenderedCurve>,
    /// Endpoints share one level, and the last bend wraps back to the start.
    closed: bool,
    /// Loop and release markers. The segments envelope turns this on.
    markers: bool,
    wave_color: Color32,
    power_color: Color32,
    fill_top_alpha: u8,
    fill_bottom_alpha: u8,
}

impl Default for SegmentsEditor {
    fn default() -> Self {
        Self {
            grid_x: 8,
            grid_y: 1,
            drag: None,
            grab: None,
            power_y: None,
            hover: None,
            menu_point: None,
            menu_power: None,
            curve: Vec::new(),
            scratch: Vec::new(),
            points: Vec::new(),
            rendered: None,
            closed: true,
            markers: false,
            wave_color: WAVE_COLOR,
            power_color: POWER_COLOR,
            fill_top_alpha: FILL_TOP_ALPHA,
            fill_bottom_alpha: FILL_BOTTOM_ALPHA,
        }
    }
}

impl SegmentsEditor {
    /// Endpoints keep their own levels.
    pub fn open() -> Self {
        Self {
            closed: false,
            ..Self::default()
        }
    }

    /// Show loop and release markers on an open curve.
    pub fn with_markers(mut self) -> Self {
        self.markers = true;
        self
    }

    /// Curve, fill, and playhead match the envelope module.
    pub fn envelope(mut self) -> Self {
        self.wave_color = Color32::from(hsva(0.3374, 0.9, 0.2, 1.0));
        self.power_color = Color32::from(hsva(0.348, 0.7, 0.4, 1.0));
        self.fill_top_alpha = 0x66;
        self.fill_bottom_alpha = 0x66;
        self
    }

    /// `phase` is the left and right playhead. Levels are read from the curve.
    pub fn ui(
        &mut self,
        ui: &mut Ui,
        segments: &mut Segments,
        phase: Option<StereoSample>,
    ) -> bool {
        let playhead = phase.map(|phase| {
            [
                (phase[0], segments.value_at(phase[0], None, None)),
                (phase[1], segments.value_at(phase[1], None, None)),
            ]
        });
        self.ui_at(ui, segments, playhead)
    }

    /// Like [`ui`](Self::ui). `playhead` is the left and right point, each `(phase, level)`.
    pub fn ui_at(
        &mut self,
        ui: &mut Ui,
        segments: &mut Segments,
        playhead: Option<[(Smp, Smp); 2]>,
    ) -> bool {
        let mut changed = false;
        let width = ui.available_width();
        let (rect, mut response) =
            ui.allocate_exact_size(vec2(width, EDITOR_HEIGHT), Sense::click());

        if rect.is_positive() {
            changed |= self.interact(ui, &response, rect, segments);
            self.paint(ui, rect, segments, playhead);
            changed |= self.context_menu(&mut response, segments);
        }

        ui.add_space(8.0);
        changed |= self.toolbar(ui, segments);
        ui.add_space(4.0);
        let help = if self.markers {
            "Double-click to add or remove a point. Drag a midpoint to bend the segment. Hold Shift to mirror that bend onto the next segment. Right-click a point to set the loop or release."
        } else {
            "Double-click to add or remove a point. Drag a midpoint to bend the segment. Hold Shift to mirror that bend onto the next segment. Right-click to flip."
        };

        ui.label(egui::RichText::new(help).small().weak());

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
        let (modifiers, now, primary_down, press_origin, press_start) = ui.input(|input| {
            (
                input.modifiers,
                input.time,
                input.pointer.button_down(PointerButton::Primary),
                input.pointer.press_origin(),
                input.pointer.press_start_time(),
            )
        });
        let pointer = response.interact_pointer_pos().or(response.hover_pos());

        if response.secondary_clicked()
            && let Some(pos) = pointer
        {
            let hit = self.hit_test(plot, segments, pos);
            self.menu_point = hit.0;
            self.menu_power = hit.1;
        }

        if response.double_clicked_by(PointerButton::Primary) {
            if let Some(pos) = pointer {
                changed |= self.double_click(plot, segments, pos, modifiers.shift);
            }
            self.end_drag();
        } else if self.drag.is_none()
            && response.is_pointer_button_down_on()
            && let Some(origin) = press_origin
        {
            let (point, power) = self.hit_test(plot, segments, origin);
            let circle = point.is_some() || power.is_some();

            if circle && let Some(pos) = CircleGrab::press(ui, pointer, origin, press_start, now) {
                if let Some(index) = point {
                    self.begin_point_drag(plot, segments, index, pos);
                } else if let Some(index) = power {
                    self.begin_power_drag(plot, index, pos);
                }
            }
        }

        let tracking = self.drag.is_some() && primary_down;

        if tracking && let Some(pos) = Self::drag_pointer(ui) {
            match self.drag {
                Some(Drag::Point(index)) => {
                    if let Some(grab) = self.grab {
                        let placed = grab.place(Self::curve_pos(plot, pos));

                        if placed != grab.center() {
                            self.move_point(segments, index, placed.x, placed.y, !modifiers.shift);
                            changed = true;
                        }
                    }
                }
                Some(Drag::Power(index)) => {
                    let y = Self::curve_pos(plot, pos).y;

                    if let Some(last) = self.power_y {
                        let delta = y - last;

                        if delta != 0.0 {
                            self.move_power(segments, index, delta, modifiers.shift);
                            changed = true;
                        }
                    }

                    self.power_y = Some(y);
                }
                None => {}
            }
        }

        if !primary_down {
            self.end_drag();
        }

        self.hover = if self.drag.is_none() && response.hovered() {
            pointer.and_then(|pos| {
                let hit = self.hit_test(plot, segments, pos);
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
                && index < segments.num_points()
            {
                let smooth = segments.point(index).smooth;
                let label = if smooth { "Flat" } else { "Smooth" };

                if ui
                    .button(label)
                    .on_hover_text("Ease this segment with a sine curve.")
                    .clicked()
                {
                    action = Some(MenuAction::SetSmooth(index, !smooth));
                    ui.close();
                }
            }

            if self.markers
                && let Some(index) = menu_point
                && index < segments.num_points()
            {
                let n = segments.num_points();

                if segments.loop_point_idx() == Some(index) {
                    if ui.button("Clear loop").clicked() {
                        action = Some(MenuAction::ClearLoop);
                        ui.close();
                    }
                } else if index + 1 < n
                    && segments
                        .release_point_idx()
                        .is_none_or(|release| index < release)
                    && ui
                        .button("Loop point")
                        .on_hover_text(
                            "Repeat from here. With no release point the loop runs to the end. With one, it runs until release while the key is held.",
                        )
                        .clicked()
                {
                    action = Some(MenuAction::SetLoop(index));
                    ui.close();
                }

                if segments.release_point_idx() == Some(index) {
                    if ui.button("Clear release").clicked() {
                        action = Some(MenuAction::ClearRelease);
                        ui.close();
                    }
                } else if segments
                    .loop_point_idx()
                    .is_none_or(|loop_point| index > loop_point)
                    && ui
                        .button("Release point")
                        .on_hover_text(
                            "Sustain this level while the key is held. On release, continue from here starting at the current level.",
                        )
                        .clicked()
                {
                    action = Some(MenuAction::SetRelease(index));
                    ui.close();
                }
            }

            if let Some(index) = menu_point
                && index > 0
                && index + 1 < segments.num_points()
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
            Some(MenuAction::Remove(index)) => segments.remove_point(index),
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
            Some(MenuAction::SetLoop(index)) => segments.set_loop_point(Some(index)),
            Some(MenuAction::ClearLoop) => segments.set_loop_point(None),
            Some(MenuAction::SetRelease(index)) => segments.set_release_point(Some(index)),
            Some(MenuAction::ClearRelease) => segments.set_release_point(None),
            None => false,
        }
    }

    fn toolbar(&mut self, ui: &mut Ui, segments: &mut Segments) -> bool {
        let mut changed = false;

        ui.horizontal_wrapped(|ui| {
            ui.label("Grid")
                .on_hover_text("Snap points while dragging. Hold Shift to place them freely.");
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

    fn paint(
        &mut self,
        ui: &mut Ui,
        plot: Rect,
        segments: &Segments,
        playhead: Option<[(Smp, Smp); 2]>,
    ) {
        let painter = ui.painter();
        let background = ui.visuals().extreme_bg_color;

        painter.rect_filled(plot, 2.0, background);
        painter.rect_stroke(
            plot,
            2.0,
            Stroke::new(1.0, Color32::from_white_alpha(32)),
            egui::StrokeKind::Inside,
        );

        self.curve_points(plot, segments);

        let loop_region = self.loop_region(plot, segments);
        self.paint_fill(painter, plot, &self.points, background, loop_region);
        Self::paint_grid(painter, plot, self.grid_x, self.grid_y);

        if self.points.len() >= 2 {
            painter.line(self.points.clone(), Stroke::new(2.0, self.wave_color));
        }

        for index in 0..segments.num_points() {
            if !self.power_active(plot, segments, index) {
                continue;
            }

            let point = self.power_position(segments, index);
            painter.circle_filled(
                Self::screen_pos(plot, point),
                POWER_RADIUS,
                self.power_color,
            );
        }

        let loop_point = if self.markers {
            segments.loop_point_idx()
        } else {
            None
        };
        let release_point = if self.markers {
            segments.release_point_idx()
        } else {
            None
        };

        for index in 0..segments.num_points() {
            let point = segments.point(index);
            let color = if Some(index) == loop_point {
                LOOP_COLOR
            } else if Some(index) == release_point {
                RELEASE_COLOR
            } else {
                POINT_COLOR
            };

            painter.circle_filled(
                Self::screen_pos(plot, pos2(point.x, point.y)),
                POINT_RADIUS,
                color,
            );
        }

        if self.markers {
            if let Some(point) = segments.loop_point() {
                Self::paint_marker(painter, plot, point, "L", LOOP_COLOR);
            }

            if let Some(point) = segments.release_point() {
                Self::paint_marker(painter, plot, point, "R", RELEASE_COLOR);
            }
        }

        if let Some(playhead) = playhead {
            self.paint_playheads(painter, plot, playhead);
        }

        let grab_stroke = Stroke::new(1.0, Color32::WHITE);

        if let Some(drag) = self.drag {
            let pos = self.target_pos(drag, plot, segments);
            let radius = Self::grab_radius(drag);
            painter.circle_filled(pos, radius, Color32::from_white_alpha(0x40));
            painter.circle_stroke(pos, radius, grab_stroke);

            if let Drag::Point(index) = drag
                && index < segments.num_points()
            {
                let level = segments.point(index).y.clamp(0.0, 1.0);
                Self::paint_level(ui, pos, Units::Normalized.format(level));
            }
        } else if let Some(hover) = self.hover {
            painter.circle_stroke(
                self.target_pos(hover, plot, segments),
                Self::grab_radius(hover),
                grab_stroke,
            );
        }
    }

    fn double_click(&self, plot: Rect, segments: &mut Segments, pos: Pos2, shift: bool) -> bool {
        let (point, power) = self.hit_test(plot, segments, pos);

        if let Some(index) = point {
            return segments.remove_point(index);
        }

        if let Some(index) = power {
            if shift {
                segments.reset_powers();
            } else {
                segments.set_power(index, 0.0);
            }
            return true;
        }

        let point = Self::curve_pos(plot, pos);
        let mut index = 0;
        let n = segments.num_points();

        while index < n && segments.point(index).x <= point.x {
            index += 1;
        }

        let smooth = if index == 0 {
            segments.point(0).smooth
        } else {
            segments.point(index - 1).smooth
        };

        segments.add_point(
            index,
            SegPoint {
                x: point.x,
                y: point.y,
                smooth,
                ..SegPoint::default()
            },
        )
    }

    fn move_point(&self, segments: &mut Segments, index: usize, x: Smp, y: Smp, snap: bool) {
        let n = segments.num_points();
        let grid_x = self.grid_x;
        let grid_y = self.grid_y;

        if index >= n {
            return;
        }

        let (min_x, max_x) = if index == 0 {
            (0.0, 0.0)
        } else if index + 1 == n {
            (1.0, 1.0)
        } else {
            (segments.point(index - 1).x, segments.point(index + 1).x)
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

        if self.closed {
            if index == 0 {
                let end = segments.point(n - 1);
                segments.set_point(n - 1, SegPoint { y, ..end });
            } else if index + 1 == n {
                let start = segments.point(0);
                segments.set_point(0, SegPoint { y, ..start });
            }
        }

        let current = segments.point(index);
        segments.set_point(index, SegPoint { x, y, ..current });
    }

    /// Bend segment `index` by a vertical pointer move.
    fn move_power(&self, segments: &mut Segments, index: usize, delta_y: Smp, sync_next: bool) {
        let n = segments.num_points();

        if index >= n {
            return;
        }

        let Some(end) = self.segment_end(segments, index) else {
            return;
        };
        let from = segments.point(index);

        if from.y == end.y {
            return;
        }

        let amount = POWER_MOUSE_MULTIPLIER * if from.y < end.y { -delta_y } else { delta_y };
        let power = sanitize(segments.power(index) + amount, -MAX_POWER, MAX_POWER);

        if segments.power(index) != power {
            segments.set_power(index, power);
        }

        if !sync_next {
            return;
        }

        let next = if index + 1 < n {
            index + 1
        } else if self.closed {
            0
        } else {
            return;
        };

        if self.segment_end(segments, next).is_some() && segments.power(next) != -power {
            segments.set_power(next, -power);
        }
    }

    fn begin_point_drag(&mut self, plot: Rect, segments: &Segments, index: usize, pointer: Pos2) {
        let point = segments.point(index);
        self.grab = Some(CircleGrab::new(
            Self::curve_pos(plot, pointer),
            pos2(point.x, point.y),
        ));
        self.drag = Some(Drag::Point(index));
    }

    fn begin_power_drag(&mut self, plot: Rect, index: usize, pointer: Pos2) {
        self.power_y = Some(Self::curve_pos(plot, pointer).y);
        self.drag = Some(Drag::Power(index));
    }

    fn end_drag(&mut self) {
        self.drag = None;
        self.grab = None;
        self.power_y = None;
    }

    fn hit_test(
        &self,
        plot: Rect,
        segments: &Segments,
        pos: Pos2,
    ) -> (Option<usize>, Option<usize>) {
        let grab_sq = GRAB_RADIUS * GRAB_RADIUS;
        let mut best_point = None;
        let mut best_distance = grab_sq;

        for index in 0..segments.num_points() {
            let point = segments.point(index);
            let distance = pos.distance_sq(Self::screen_pos(plot, pos2(point.x, point.y)));

            if distance < best_distance {
                best_distance = distance;
                best_point = Some(index);
            }
        }

        if best_point.is_some() {
            return (best_point, None);
        }

        let mut best_power = None;
        best_distance = POWER_GRAB_RADIUS * POWER_GRAB_RADIUS;

        for index in 0..segments.num_points() {
            if !self.power_active(plot, segments, index) {
                continue;
            }

            let point = self.power_position(segments, index);
            let distance = pos.distance_sq(Self::screen_pos(plot, point));

            if distance < best_distance {
                best_distance = distance;
                best_power = Some(index);
            }
        }

        (None, best_power)
    }

    fn power_active(&self, plot: Rect, segments: &Segments, index: usize) -> bool {
        let Some(end) = self.segment_end(segments, index) else {
            return false;
        };
        let from = segments.point(index);
        let dx = end.x - from.x;
        let dy = end.y - from.y;

        plot.width() * dx >= MIN_POWER_WIDTH && dy != 0.0
    }

    fn segment_end(&self, segments: &Segments, index: usize) -> Option<Pos2> {
        let n = segments.num_points();

        if index >= n {
            return None;
        }

        if index + 1 < n {
            let point = segments.point(index + 1);
            Some(pos2(point.x, point.y))
        } else if self.closed {
            let point = segments.point(0);
            Some(pos2(point.x + 1.0, point.y))
        } else {
            None
        }
    }

    fn power_position(&self, segments: &Segments, index: usize) -> Pos2 {
        let from = segments.point(index);
        let Some(end) = self.segment_end(segments, index) else {
            return pos2(from.x, from.y);
        };
        let mut x = (from.x + end.x) * 0.5;

        if x >= 1.0 {
            x -= 1.0;
        }

        let t = power_scale(0.5, segments.power(index));
        let y = from.y + t * (end.y - from.y);

        pos2(x, y)
    }

    /// Closed polyline for one sample per horizontal point, with each breakpoint kept.
    ///
    /// `render_points` runs again when [`Segments::revision`] or the plot size changes.
    /// The polyline is projected into `plot` on every call.
    fn curve_points(&mut self, plot: Rect, segments: &Segments) {
        if plot.width() <= 0.0 {
            self.points.clear();
            self.rendered = None;
            return;
        }

        let size = plot.size();
        let revision = segments.revision();

        if !self
            .rendered
            .as_ref()
            .is_some_and(|rendered| rendered.revision == revision && rendered.size == size)
        {
            let steps = size.x.round().max(1.0) as usize;

            segments.render_points(steps, &mut self.curve, &mut self.scratch);
            self.rendered = Some(RenderedCurve { revision, size });
        }

        self.points.clear();
        self.points.reserve(self.curve.len());

        for point in &self.curve {
            self.points.push(Self::screen_pos(plot, (*point).into()));
        }
    }

    /// Left stays the curve color. The right channel keeps that saturation and value.
    fn paint_playheads(&self, painter: &Painter, plot: Rect, playhead: [(Smp, Smp); 2]) {
        let left = Self::playhead_pos(plot, playhead[0].0, playhead[0].1);
        let right = Self::playhead_pos(plot, playhead[1].0, playhead[1].1);

        if left.distance_sq(right) > 0.25 {
            painter.circle_filled(
                right,
                PLAYHEAD_RADIUS,
                Self::right_playhead_color(self.wave_color),
            );
        }

        painter.circle_filled(left, PLAYHEAD_RADIUS, self.wave_color);
    }

    fn right_playhead_color(left: Color32) -> Color32 {
        let mut color = Hsva::from(left);
        color.h = RIGHT_PLAYHEAD_HUE;
        Color32::from(color)
    }

    /// Screen position of the playhead at `(phase, level)`.
    fn playhead_pos(plot: Rect, phase: Smp, level: Smp) -> Pos2 {
        let phase = sanitize(phase, 0.0, 1.0);
        let level = sanitize(level, 0.0, 1.0);

        Self::screen_pos(plot, pos2(phase, level))
    }

    fn paint_marker(painter: &Painter, plot: Rect, point: SegPoint, label: &str, color: Color32) {
        let pos = Self::screen_pos(plot, pos2(point.x, point.y));
        let galley = painter.layout_no_wrap(
            label.to_owned(),
            FontId::new(10.0, FontFamily::Name("Bold".into())),
            color,
        );
        let x = pos.x - galley.size().x * 0.5;
        let above = pos.y - POINT_RADIUS - galley.size().y - 3.0;
        let y = if above >= plot.top() {
            above
        } else {
            pos.y + POINT_RADIUS + 3.0
        };

        painter.galley(pos2(x, y), galley, color);
    }

    /// Pointer position while a drag is held, including outside the plot and the window.
    fn drag_pointer(ui: &Ui) -> Option<Pos2> {
        let mut pos = ui.input(|input| input.pointer.latest_pos())?;

        if let Some(from_global) = ui.ctx().layer_transform_from_global(ui.layer_id()) {
            pos = from_global * pos;
        }

        Some(pos)
    }

    /// Plot position of a screen point. Values outside 0–1 are past the canvas.
    fn curve_pos(plot: Rect, pos: Pos2) -> Pos2 {
        pos2(
            (pos.x - plot.left()) / plot.width(),
            (plot.bottom() - pos.y) / plot.height(),
        )
    }

    fn target_pos(&self, target: Drag, plot: Rect, segments: &Segments) -> Pos2 {
        match target {
            Drag::Point(index) => {
                let point = segments.point(index);
                Self::screen_pos(plot, pos2(point.x, point.y))
            }
            Drag::Power(index) => Self::screen_pos(plot, self.power_position(segments, index)),
        }
    }

    fn paint_level(ui: &Ui, anchor: Pos2, text: String) {
        let text_color = Color32::from(LABEL_TEXT_COLOR);
        let bg_color = Color32::from(LABEL_BG_COLOR);
        let border_color = Color32::from(LABEL_BORDER_COLOR);
        let painter = ui.painter();
        let galley = painter.layout_no_wrap(
            text,
            FontId::new(11.0, FontFamily::Name("Bold".into())),
            text_color,
        );
        let padding = vec2(LABEL_PADDING, LABEL_PADDING * 0.5);
        let box_size = galley.size() + padding * 2.0;
        let bounds = ui.clip_rect();
        let above = anchor.y - GRAB_RADIUS - box_size.y - LABEL_POINTER_GAP;
        let top = if above >= bounds.top() {
            above
        } else {
            anchor.y + GRAB_RADIUS + LABEL_POINTER_GAP
        };
        let top = top.clamp(
            bounds.top(),
            (bounds.bottom() - box_size.y).max(bounds.top()),
        );
        let left = (anchor.x - box_size.x * 0.5).clamp(
            bounds.left(),
            (bounds.right() - box_size.x).max(bounds.left()),
        );
        let box_rect = Rect::from_min_size(pos2(left, top), box_size);
        let fg_painter = painter.clone().with_layer_id(LayerId::new(
            Order::Foreground,
            Id::new("segments-point-label"),
        ));

        fg_painter.rect(
            box_rect,
            LABEL_CORNER_RADIUS,
            bg_color,
            Stroke::new(1.0, border_color),
            StrokeKind::Inside,
        );
        fg_painter.galley(box_rect.min + padding, galley, text_color);
    }

    fn grab_radius(target: Drag) -> f32 {
        match target {
            Drag::Point(_) => GRAB_RADIUS,
            Drag::Power(_) => POWER_GRAB_RADIUS,
        }
    }

    fn screen_pos(plot: Rect, point: Pos2) -> Pos2 {
        pos2(
            plot.left() + point.x.clamp(0.0, 1.0) * plot.width(),
            plot.bottom() - point.y.clamp(0.0, 1.0) * plot.height(),
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

    /// Screen span of the loop, from the loop point to the release point or the end.
    fn loop_region(&self, plot: Rect, segments: &Segments) -> Option<Rect> {
        if !self.markers {
            return None;
        }

        let start = segments.loop_point()?;
        let end_x = segments.release_point().map(|point| point.x).unwrap_or(1.0);
        let left = plot.left() + start.x.clamp(0.0, 1.0) * plot.width();
        let right = plot.left() + end_x.clamp(0.0, 1.0) * plot.width();

        if right <= left {
            None
        } else {
            Some(Rect::from_min_max(
                pos2(left, plot.top()),
                pos2(right, plot.bottom()),
            ))
        }
    }

    /// Curve gradient first, masked above the curve. The loop band is painted
    /// after that, for the full plot height.
    ///
    /// One gradient rectangle, then an opaque cover above the curve.
    /// A per-column mesh creases the gradient: vertical lines from premultiplied
    /// vertex colors, horizontal lines from sampling a gradient texture.
    fn paint_fill(
        &self,
        painter: &Painter,
        plot: Rect,
        points: &[Pos2],
        background: Color32,
        loop_region: Option<Rect>,
    ) {
        if plot.height() <= 0.0 || plot.width() <= 0.0 {
            return;
        }

        let top = self.fill_top_alpha;
        let bottom = self.fill_bottom_alpha;
        let background = Color32::from_rgb(background.r(), background.g(), background.b());
        let mut gradient = Mesh::default();

        Self::paint_wave_span(
            &mut gradient,
            plot,
            plot.left(),
            plot.right(),
            self.wave_color,
            top,
            bottom,
        );

        if !gradient.is_empty() {
            painter.add(Shape::mesh(gradient));
        }

        if points.len() >= 2 {
            let mut mask = Mesh::default();

            for window in points.windows(2) {
                let start = window[0];
                let end = window[1];

                if end.x <= start.x {
                    continue;
                }

                Self::add_cover(&mut mask, plot.top(), start, end, background);
            }

            if !mask.is_empty() {
                painter.add(Shape::mesh(mask));
            }
        }

        if let Some(region) = loop_region {
            let mut loop_fill = Mesh::default();

            let loop_fill_color = Color32::from(LOOP_FILL_COLOR);

            Self::paint_wave_span(
                &mut loop_fill,
                plot,
                region.left(),
                region.right(),
                loop_fill_color,
                loop_fill_color.a(),
                loop_fill_color.a(),
            );

            if !loop_fill.is_empty() {
                painter.add(Shape::mesh(loop_fill));
            }
        }
    }

    fn add_cover(mesh: &mut Mesh, top: f32, start: Pos2, end: Pos2, cover: Color32) {
        if end.x <= start.x {
            return;
        }

        let index = mesh.vertices.len() as u32;

        mesh.colored_vertex(pos2(start.x, top), cover);
        mesh.colored_vertex(pos2(end.x, top), cover);
        mesh.colored_vertex(end, cover);
        mesh.colored_vertex(start, cover);
        mesh.add_triangle(index, index + 1, index + 2);
        mesh.add_triangle(index, index + 2, index + 3);
    }

    fn paint_wave_span(
        mesh: &mut Mesh,
        plot: Rect,
        left: f32,
        right: f32,
        color: Color32,
        top_alpha: u8,
        bottom_alpha: u8,
    ) {
        if right <= left {
            return;
        }

        let top = Self::with_alpha(color, top_alpha);
        let bottom = Self::with_alpha(color, bottom_alpha);
        let index = mesh.vertices.len() as u32;

        mesh.colored_vertex(pos2(left, plot.top()), top);
        mesh.colored_vertex(pos2(right, plot.top()), top);
        mesh.colored_vertex(pos2(right, plot.bottom()), bottom);
        mesh.colored_vertex(pos2(left, plot.bottom()), bottom);
        mesh.add_triangle(index, index + 1, index + 2);
        mesh.add_triangle(index, index + 2, index + 3);
    }

    fn with_alpha(color: Color32, alpha: u8) -> Color32 {
        Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha)
    }
}

#[cfg(test)]
mod tests;
