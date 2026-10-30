use egui::{
    Color32, CursorIcon, DragValue, FontFamily, FontId, LayerId, Mesh, Order, Painter,
    PointerButton, Pos2, Rect, Response, Sense, Shape, Stroke, StrokeKind, Ui, Vec2, ecolor::Hsva,
    layers::ShapeIdx, pos2, vec2,
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
/// Right playhead draws once it is more than half a pixel from the left.
const PLAYHEAD_OVERLAP_SQ: f32 = 0.25;
/// Yellow. Saturation and value are taken from the left playhead.
const RIGHT_PLAYHEAD_HUE: f32 = 1.0 / 6.0;
const POWER_RADIUS: f32 = 3.0;
const MIN_POWER_WIDTH: f32 = 3.0;
const SNAP_X: Smp = 0.02;
const SNAP_Y: Smp = 0.04;
/// Power change for a pointer move of one editor height. Matches Vital's line editor.
const POWER_MOUSE_MULTIPLIER: Smp = 9.0;
/// A press stays a click until the pointer moves this far.
const MAX_CLICK_DIST: f32 = 2.0;
/// A press stays a click until it has been held this long.
const MAX_CLICK_DURATION: f64 = 0.2;

const WAVE_COLOR: Color32 = Color32::from_rgb(0x4a, 0xb0, 0xff);
const LOOP_COLOR: Color32 = Color32::from_rgb(0xf0, 0xc0, 0x4a);
const LOOP_FILL_COLOR: Hsva = hsva(0.0, 0.9092, 1.0, 0.25);
const RELEASE_COLOR: Color32 = Color32::from_rgb(0xff, 0x7a, 0x59);
const FILL_TOP_ALPHA: u8 = 0x47;
const FILL_BOTTOM_ALPHA: u8 = 0x28;
const ENVELOPE_WAVE: Hsva = hsva(0.3374, 0.9, 0.2, 1.0);
const ENVELOPE_POWER: Hsva = hsva(0.348, 0.7, 0.4, 1.0);
const ENVELOPE_FILL_ALPHA: u8 = 0x66;
const GRID_COLOR: Color32 = Color32::from_rgba_unmultiplied_const(255, 255, 255, 28);
const POINT_COLOR: Color32 = Color32::from_rgb(0xe8, 0xf4, 0xff);
const POWER_COLOR: Color32 = Color32::from_rgb(0x9a, 0xd0, 0xff);
const LABEL_BG_COLOR: Hsva = hsva(0.115, 0.05, 0.01, 1.0);
const LABEL_BORDER_COLOR: Hsva = hsva(0.115, 0.05, 0.2, 1.0);
const LABEL_TEXT_COLOR: Hsva = hsva(0.115, 0.05, 0.5, 1.0);
const LABEL_PADDING: f32 = 6.0;
const LABEL_POINTER_GAP: f32 = 4.0;
const LABEL_CORNER_RADIUS: f32 = 2.0;

const HELP: &str = "Double-click to add or remove a point. Drag a midpoint to bend the segment. Hold Shift to mirror that bend onto the next segment. Right-click to flip.";
const HELP_MARKERS: &str = "Double-click to add or remove a point. Drag a midpoint to bend the segment. Hold Shift to mirror that bend onto the next segment. Right-click a point to set the loop or release.";
const LOOP_TIP: &str = "Repeat from here. With no release point the loop runs to the end. With one, it runs until release while the key is held.";
const RELEASE_TIP: &str = "Sustain this level while the key is held. On release, continue from here starting at the current level.";

type Preset = (&'static str, fn() -> Segments);

const PRESETS: &[Preset] = &[
    ("Triangle", segment_presets::triangle),
    ("Square", segment_presets::square),
    ("Sine", segment_presets::sine),
    ("Saw up", segment_presets::saw_up),
    ("Saw down", segment_presets::saw_down),
];

/// Point or bend handle under the pointer.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Drag {
    Point(usize),
    Power(usize),
}

impl Drag {
    fn radius(self) -> f32 {
        match self {
            Self::Point(_) => GRAB_RADIUS,
            Self::Power(_) => POWER_GRAB_RADIUS,
        }
    }
}

/// Revision and plot size the cached curve samples were built from.
struct CurveCache {
    revision: u64,
    size: Vec2,
}

pub struct SegmentsEditor {
    grid_x: i32,
    grid_y: i32,
    drag: Option<Drag>,
    /// Offset that keeps the dragged point under the pointer.
    grab: Option<CircleGrab>,
    /// Curve-space pointer y at the previous bend update.
    power_y: Option<Smp>,
    /// Curve samples in 0–1 space.
    curve: Vec<[Smp; 2]>,
    /// Workspace for [`Segments::render_points`].
    scratch: Vec<Smp>,
    /// Cached curve samples projected into the current plot.
    points: Vec<Pos2>,
    curve_cache: Option<CurveCache>,
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
            curve: Vec::new(),
            scratch: Vec::new(),
            points: Vec::new(),
            curve_cache: None,
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
        self.wave_color = Color32::from(ENVELOPE_WAVE);
        self.power_color = Color32::from(ENVELOPE_POWER);
        self.fill_top_alpha = ENVELOPE_FILL_ALPHA;
        self.fill_bottom_alpha = ENVELOPE_FILL_ALPHA;
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
    ///
    /// Returns whether the curve revision changed while the editor was shown.
    pub fn ui_at(
        &mut self,
        ui: &mut Ui,
        segments: &mut Segments,
        playhead: Option<[(Smp, Smp); 2]>,
    ) -> bool {
        let revision = segments.revision();
        let width = ui.available_width();
        let (plot, response) = ui.allocate_exact_size(vec2(width, EDITOR_HEIGHT), Sense::click());

        if plot.is_positive() {
            // Reserved before the handles. Filled in after they edit the curve,
            // so the line stays behind them and still matches this frame.
            let curve_slot = ui.painter().with_clip_rect(plot).add(Shape::Noop);

            self.canvas(&response, plot, segments);

            let count = segments.num_points();

            for index in 0..count {
                self.bend(ui, plot, segments, index);
            }

            let mut remove = None;

            for index in 0..count {
                if self.point(ui, plot, segments, index) {
                    remove = Some(index);
                }
            }

            if let Some(index) = remove {
                segments.remove_point(index);
            }

            if let Some(playhead) = playhead {
                self.paint_playheads(ui.painter(), plot, playhead);
            }

            self.paint_curve_behind(ui, plot, segments, curve_slot);
        }

        ui.add_space(8.0);
        self.toolbar(ui, segments);
        ui.add_space(4.0);
        ui.label(egui::RichText::new(self.help()).small().weak());

        segments.revision() != revision
    }

    fn help(&self) -> &'static str {
        if self.markers { HELP_MARKERS } else { HELP }
    }

    /// Empty plot: double-click inserts a point, right-click flips the curve.
    fn canvas(&mut self, response: &Response, plot: Rect, segments: &mut Segments) {
        self.canvas_menu(response, segments);

        if response.double_clicked_by(PointerButton::Primary)
            && let Some(pos) = response.interact_pointer_pos().or(response.hover_pos())
        {
            self.add_point(plot, segments, pos);
        }
    }

    /// One bend handle: drag, double-click, menu, and paint.
    fn bend(&mut self, ui: &Ui, plot: Rect, segments: &mut Segments, index: usize) {
        let active = self.power_active(plot, segments, index);

        if !active {
            if self.drag == Some(Drag::Power(index)) {
                self.end_drag();
            }

            return;
        }

        let response = Self::handle_response(
            ui,
            plot,
            Self::screen_pos(plot, self.power_position(segments, index)),
            POWER_GRAB_RADIUS,
            ("segments-bend", index),
        );
        self.power_menu(&response, segments, index);

        if response.double_clicked_by(PointerButton::Primary) {
            let shift = ui.input(|input| input.modifiers.shift);

            if shift {
                segments.reset_powers();
            } else {
                segments.set_power(index, 0.0);
            }

            self.end_drag();
        } else {
            self.bend_drag(ui, plot, segments, index, &response);
        }

        let center = Self::screen_pos(plot, self.power_position(segments, index));
        let dragging = self.drag == Some(Drag::Power(index));

        ui.painter()
            .circle_filled(center, POWER_RADIUS, self.power_color);
        Self::paint_grab(ui, &response, center, POWER_GRAB_RADIUS, dragging);
    }

    /// One breakpoint: drag, double-click, menu, and paint.
    ///
    /// `true` asks the caller to remove this point after the rest of the points are drawn.
    fn point(&mut self, ui: &Ui, plot: Rect, segments: &mut Segments, index: usize) -> bool {
        let response = Self::handle_response(
            ui,
            plot,
            Self::point_screen(plot, segments.point(index)),
            GRAB_RADIUS,
            ("segments-point", index),
        );
        let mut remove = self.point_menu(&response, segments, index);

        if response.double_clicked_by(PointerButton::Primary) {
            self.end_drag();

            if index > 0 && index + 1 < segments.num_points() {
                remove = true;
            }
        } else if !remove {
            self.point_drag(ui, plot, segments, index, &response);
        }

        if remove {
            self.end_drag();
        } else {
            self.paint_point(ui, plot, segments, index, &response);
        }

        remove
    }

    /// Shift places the point freely.
    fn point_drag(
        &mut self,
        ui: &Ui,
        plot: Rect,
        segments: &mut Segments,
        index: usize,
        response: &Response,
    ) {
        if let Some(pos) = self.drag_begin(ui, response) {
            let point = segments.point(index);

            self.grab = Some(CircleGrab::new(
                Self::curve_pos(plot, pos),
                pos2(point.x, point.y),
            ));
            self.drag = Some(Drag::Point(index));
        }

        let Some(pos) = self.drag_pos(ui, Drag::Point(index)) else {
            return;
        };
        let snap = !ui.input(|input| input.modifiers.shift);

        self.move_grabbed_point(segments, index, plot, pos, snap);
    }

    /// Shift mirrors the bend onto the next segment.
    fn bend_drag(
        &mut self,
        ui: &Ui,
        plot: Rect,
        segments: &mut Segments,
        index: usize,
        response: &Response,
    ) {
        if let Some(pos) = self.drag_begin(ui, response) {
            self.power_y = Some(Self::curve_pos(plot, pos).y);
            self.drag = Some(Drag::Power(index));
        }

        let Some(pos) = self.drag_pos(ui, Drag::Power(index)) else {
            return;
        };
        let sync_next = ui.input(|input| input.modifiers.shift);

        self.drag_power(segments, index, plot, pos, sync_next);
    }

    /// Pointer once this press is a drag and no other handle is held.
    ///
    /// While it can still be a click, a repaint is requested so the hold can elapse.
    fn drag_begin(&self, ui: &Ui, response: &Response) -> Option<Pos2> {
        if self.drag.is_some() || !response.is_pointer_button_down_on() {
            return None;
        }

        let (now, origin, press_start) = ui.input(|input| {
            (
                input.time,
                input.pointer.press_origin(),
                input.pointer.press_start_time(),
            )
        });
        let origin = origin?;
        let pointer = response.interact_pointer_pos().or(response.hover_pos());
        let moved = pointer.is_some_and(|pos| pos.distance(origin) > MAX_CLICK_DIST);
        let held = press_start.is_some_and(|start| now - start > MAX_CLICK_DURATION);

        if (moved || held)
            && let Some(pos) = pointer
        {
            return Some(pos);
        }

        let remaining = press_start
            .map(|start| (MAX_CLICK_DURATION - (now - start)) as f32)
            .unwrap_or(MAX_CLICK_DURATION as f32);

        ui.ctx().request_repaint_after_secs(remaining.max(0.0));
        None
    }

    /// Pointer while `drag` is held. Releasing the button ends it.
    fn drag_pos(&mut self, ui: &Ui, drag: Drag) -> Option<Pos2> {
        if self.drag != Some(drag) {
            return None;
        }

        if !ui.input(|input| input.pointer.primary_down()) {
            self.end_drag();
            return None;
        }

        Self::drag_pointer(ui)
    }

    fn handle_response(
        ui: &Ui,
        plot: Rect,
        center: Pos2,
        radius: f32,
        id: impl std::hash::Hash + std::fmt::Debug,
    ) -> Response {
        let rect = Rect::from_center_size(center, vec2(radius * 2.0, radius * 2.0)).intersect(plot);
        let rect = if rect.is_positive() {
            rect
        } else {
            Rect::from_center_size(center, vec2(1.0, 1.0))
        };

        ui.interact(rect, ui.make_persistent_id(id), Sense::click_and_drag())
    }

    fn move_grabbed_point(
        &mut self,
        segments: &mut Segments,
        index: usize,
        plot: Rect,
        pos: Pos2,
        snap: bool,
    ) {
        let Some(grab) = self.grab else {
            return;
        };
        let placed = grab.place(Self::curve_pos(plot, pos));

        if placed == grab.center() {
            return;
        }

        self.move_point(segments, index, placed.x, placed.y, snap);
    }

    /// Bend follows vertical pointer travel. `sync_next` mirrors the bend onto the next segment.
    fn drag_power(
        &mut self,
        segments: &mut Segments,
        index: usize,
        plot: Rect,
        pos: Pos2,
        sync_next: bool,
    ) {
        let y = Self::curve_pos(plot, pos).y;

        if let Some(last) = self.power_y {
            let delta = y - last;

            if delta != 0.0 {
                self.move_power(segments, index, delta, sync_next);
            }
        }

        self.power_y = Some(y);
    }

    fn end_drag(&mut self) {
        self.drag = None;
        self.grab = None;
        self.power_y = None;
    }

    fn add_point(&self, plot: Rect, segments: &mut Segments, pos: Pos2) {
        let point = Self::curve_pos(plot, pos);
        let index = insertion_index(segments, point.x);
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
        );
    }

    fn move_point(&self, segments: &mut Segments, index: usize, x: Smp, y: Smp, snap: bool) {
        if index >= segments.num_points() {
            return;
        }

        let (min_x, max_x) = x_limits(segments, index);
        let mut x = x.clamp(min_x, max_x);
        let mut y = y.clamp(0.0, 1.0);

        if snap {
            x = snap_near(x, self.grid_x, SNAP_X, min_x, max_x);
            y = snap_near(y, self.grid_y, SNAP_Y, 0.0, 1.0);
        }

        self.match_closed_ends(segments, index, y);

        let current = segments.point(index);
        segments.set_point(index, SegPoint { x, y, ..current });
    }

    /// Closed curves keep the first and last levels equal.
    fn match_closed_ends(&self, segments: &mut Segments, index: usize, y: Smp) {
        if !self.closed {
            return;
        }

        let n = segments.num_points();
        let other = if index == 0 {
            n - 1
        } else if index + 1 == n {
            0
        } else {
            return;
        };
        let point = segments.point(other);

        segments.set_point(other, SegPoint { y, ..point });
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

        let Some(next) = self.mirrored_bend(index, n) else {
            return;
        };

        if self.segment_end(segments, next).is_some() && segments.power(next) != -power {
            segments.set_power(next, -power);
        }
    }

    /// Next segment that receives the mirrored bend. The last segment of a closed curve wraps.
    fn mirrored_bend(&self, index: usize, n: usize) -> Option<usize> {
        if index + 1 < n {
            Some(index + 1)
        } else if self.closed {
            Some(0)
        } else {
            None
        }
    }

    /// `true` when the menu asked to remove this point. Other choices edit `segments` directly.
    fn point_menu(&self, response: &Response, segments: &mut Segments, index: usize) -> bool {
        let markers = self.markers;
        let mut remove = false;

        response.context_menu(|ui| {
            if index < segments.num_points() {
                let smooth = segments.point(index).smooth;
                let label = if smooth { "Flat" } else { "Smooth" };

                if ui
                    .button(label)
                    .on_hover_text("Ease this segment with a sine curve.")
                    .clicked()
                {
                    segments.set_smooth(index, !smooth);
                    ui.close();
                }
            }

            if markers && index < segments.num_points() {
                let n = segments.num_points();
                let can_loop = index + 1 < n
                    && segments
                        .release_point_idx()
                        .is_none_or(|release| index < release);
                let can_release = segments
                    .loop_point_idx()
                    .is_none_or(|loop_point| index > loop_point);

                if segments.loop_point_idx() == Some(index) {
                    if ui.button("Clear loop").clicked() {
                        segments.set_loop_point(None);
                        ui.close();
                    }
                } else if can_loop && ui.button("Loop point").on_hover_text(LOOP_TIP).clicked() {
                    segments.set_loop_point(Some(index));
                    ui.close();
                }

                if segments.release_point_idx() == Some(index) {
                    if ui.button("Clear release").clicked() {
                        segments.set_release_point(None);
                        ui.close();
                    }
                } else if can_release
                    && ui
                        .button("Release point")
                        .on_hover_text(RELEASE_TIP)
                        .clicked()
                {
                    segments.set_release_point(Some(index));
                    ui.close();
                }
            }

            if index > 0 && index + 1 < segments.num_points() && ui.button("Remove point").clicked()
            {
                remove = true;
                ui.close();
            }
        });

        remove
    }

    fn power_menu(&self, response: &Response, segments: &mut Segments, index: usize) {
        response.context_menu(|ui| {
            if index < segments.num_points() {
                let smooth = segments.point(index).smooth;
                let label = if smooth { "Flat" } else { "Smooth" };

                if ui
                    .button(label)
                    .on_hover_text("Ease this segment with a sine curve.")
                    .clicked()
                {
                    segments.set_smooth(index, !smooth);
                    ui.close();
                }
            }

            if ui.button("Reset bend").clicked() {
                segments.set_power(index, 0.0);
                ui.close();
            }
        });
    }

    fn canvas_menu(&self, response: &Response, segments: &mut Segments) {
        response.context_menu(|ui| {
            if ui.button("Flip horizontal").clicked() {
                segments.flip_horizontal();
                ui.close();
            }

            if ui.button("Flip vertical").clicked() {
                segments.flip_vertical();
                ui.close();
            }
        });
    }

    fn toolbar(&mut self, ui: &mut Ui, segments: &mut Segments) {
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
                }
            }
        });
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

    /// Next breakpoint in curve space. The closing segment ends one cycle past the start.
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

    /// Midpoint in phase, power-scaled in level. A wrap past 1 folds back into the plot.
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
            self.curve_cache = None;
            return;
        }

        let size = plot.size();
        let revision = segments.revision();
        let fresh = self
            .curve_cache
            .as_ref()
            .is_some_and(|cache| cache.revision == revision && cache.size == size);

        if !fresh {
            let steps = size.x.round().max(1.0) as usize;

            segments.render_points(steps, &mut self.curve, &mut self.scratch);
            self.curve_cache = Some(CurveCache { revision, size });
        }

        self.points.clear();
        self.points.reserve(self.curve.len());

        for point in &self.curve {
            self.points.push(Self::screen_pos(plot, (*point).into()));
        }
    }

    /// Move the curve shapes into `slot`, which was reserved before the handles.
    fn paint_curve_behind(&mut self, ui: &Ui, plot: Rect, segments: &Segments, slot: ShapeIdx) {
        let layer_id = ui.layer_id();
        let clip = ui.painter().with_clip_rect(plot).clip_rect();
        let first = ui.ctx().graphics(|gfx| {
            gfx.get(layer_id)
                .map(|list| list.all_entries().len())
                .unwrap_or(0)
        });

        self.paint_scene(ui, plot, segments);

        ui.ctx().graphics_mut(|gfx| {
            let Some(list) = gfx.get_mut(layer_id) else {
                return;
            };
            let end = list.all_entries().len();
            let mut shapes = Vec::with_capacity(end.saturating_sub(first));

            for index in first..end {
                list.mutate_shape(ShapeIdx(index), |clipped| {
                    shapes.push(std::mem::replace(&mut clipped.shape, Shape::Noop));
                });
            }

            list.set(slot, clip, Shape::Vec(shapes));
        });
    }

    fn paint_scene(&mut self, ui: &Ui, plot: Rect, segments: &Segments) {
        let painter = ui.painter().with_clip_rect(plot);
        let background = ui.visuals().extreme_bg_color;

        self.paint_plot(&painter, plot, background);
        self.curve_points(plot, segments);

        let loop_region = self.loop_region(plot, segments);
        self.paint_fill(&painter, plot, &self.points, background, loop_region);
        Self::paint_grid(&painter, plot, self.grid_x, self.grid_y);
        self.paint_curve(&painter);
    }

    fn paint_plot(&self, painter: &Painter, plot: Rect, background: Color32) {
        painter.rect_filled(plot, 2.0, background);
        painter.rect_stroke(
            plot,
            2.0,
            Stroke::new(1.0, Color32::from_white_alpha(32)),
            StrokeKind::Inside,
        );
    }

    fn paint_curve(&self, painter: &Painter) {
        if self.points.len() >= 2 {
            painter.line(self.points.clone(), Stroke::new(2.0, self.wave_color));
        }
    }

    fn paint_point(
        &self,
        ui: &Ui,
        plot: Rect,
        segments: &Segments,
        index: usize,
        response: &Response,
    ) {
        let point = segments.point(index);
        let center = Self::point_screen(plot, point);
        let painter = ui.painter();

        painter.circle_filled(center, POINT_RADIUS, self.point_color(segments, index));

        if self.markers && segments.loop_point_idx() == Some(index) {
            Self::paint_marker(painter, plot, point, "L", LOOP_COLOR);
        }

        if self.markers && segments.release_point_idx() == Some(index) {
            Self::paint_marker(painter, plot, point, "R", RELEASE_COLOR);
        }

        let dragging = self.drag == Some(Drag::Point(index));
        Self::paint_grab(ui, response, center, Drag::Point(index).radius(), dragging);

        if dragging {
            let level = point.y.clamp(0.0, 1.0);
            Self::paint_level(ui, center, Units::Normalized.format(level));
        }
    }

    fn point_color(&self, segments: &Segments, index: usize) -> Color32 {
        if !self.markers {
            return POINT_COLOR;
        }

        if segments.loop_point_idx() == Some(index) {
            LOOP_COLOR
        } else if segments.release_point_idx() == Some(index) {
            RELEASE_COLOR
        } else {
            POINT_COLOR
        }
    }

    fn paint_grab(ui: &Ui, response: &Response, center: Pos2, radius: f32, dragging: bool) {
        if dragging {
            ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
        } else {
            response.clone().on_hover_cursor(CursorIcon::Grab);
        }

        if !dragging && !response.hovered() {
            return;
        }

        let painter = ui.painter().clone().with_layer_id(Self::handle_layer(ui));
        let stroke = Stroke::new(1.0, Color32::WHITE);

        if dragging {
            painter.circle_filled(center, radius, Color32::from_white_alpha(0x40));
        }

        painter.circle_stroke(center, radius, stroke);
    }

    fn handle_layer(ui: &Ui) -> LayerId {
        LayerId::new(Order::Foreground, ui.id().with("segments-handle"))
    }

    /// Left stays the curve color. The right channel keeps that saturation and value.
    fn paint_playheads(&self, painter: &Painter, plot: Rect, playhead: [(Smp, Smp); 2]) {
        let left = Self::playhead_pos(plot, playhead[0].0, playhead[0].1);
        let right = Self::playhead_pos(plot, playhead[1].0, playhead[1].1);

        if left.distance_sq(right) > PLAYHEAD_OVERLAP_SQ {
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
        let pos = Self::point_screen(plot, point);
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

    fn paint_level(ui: &Ui, anchor: Pos2, text: String) {
        let text_color = Color32::from(LABEL_TEXT_COLOR);
        let bg_color = Color32::from(LABEL_BG_COLOR);
        let border_color = Color32::from(LABEL_BORDER_COLOR);
        let painter = ui.painter().clone().with_layer_id(Self::handle_layer(ui));
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

        painter.rect(
            box_rect,
            LABEL_CORNER_RADIUS,
            bg_color,
            Stroke::new(1.0, border_color),
            StrokeKind::Inside,
        );
        painter.galley(box_rect.min + padding, galley, text_color);
    }

    fn point_screen(plot: Rect, point: SegPoint) -> Pos2 {
        Self::screen_pos(plot, pos2(point.x, point.y))
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

    /// Gradient under the curve, then the loop band on top.
    ///
    /// The wave is one vertical gradient. An opaque cover hides everything above
    /// the polyline. One quad keeps that gradient smooth.
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

        self.paint_gradient(painter, plot);
        Self::paint_cover(painter, plot, points, background);

        if let Some(region) = loop_region {
            Self::paint_loop_band(painter, plot, region);
        }
    }

    fn paint_gradient(&self, painter: &Painter, plot: Rect) {
        let mut mesh = Mesh::default();

        Self::add_gradient(
            &mut mesh,
            plot,
            plot.left(),
            plot.right(),
            self.wave_color,
            self.fill_top_alpha,
            self.fill_bottom_alpha,
        );

        if !mesh.is_empty() {
            painter.add(Shape::mesh(mesh));
        }
    }

    fn paint_cover(painter: &Painter, plot: Rect, points: &[Pos2], background: Color32) {
        if points.len() < 2 {
            return;
        }

        let cover = Color32::from_rgb(background.r(), background.g(), background.b());
        let mut mesh = Mesh::default();

        for window in points.windows(2) {
            let start = window[0];
            let end = window[1];

            if end.x <= start.x {
                continue;
            }

            Self::add_cover(&mut mesh, plot.top(), start, end, cover);
        }

        if !mesh.is_empty() {
            painter.add(Shape::mesh(mesh));
        }
    }

    fn paint_loop_band(painter: &Painter, plot: Rect, region: Rect) {
        let mut mesh = Mesh::default();
        let color = Color32::from(LOOP_FILL_COLOR);

        Self::add_gradient(
            &mut mesh,
            plot,
            region.left(),
            region.right(),
            color,
            color.a(),
            color.a(),
        );

        if !mesh.is_empty() {
            painter.add(Shape::mesh(mesh));
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

    fn add_gradient(
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

/// First point whose phase is past `x`, or the end of the curve.
fn insertion_index(segments: &Segments, x: Smp) -> usize {
    (0..segments.num_points())
        .find(|&index| segments.point(index).x > x)
        .unwrap_or(segments.num_points())
}

/// Phase range a point may occupy. Endpoints stay pinned to 0 and 1.
fn x_limits(segments: &Segments, index: usize) -> (Smp, Smp) {
    let n = segments.num_points();

    if index == 0 {
        (0.0, 0.0)
    } else if index + 1 == n {
        (1.0, 1.0)
    } else {
        (segments.point(index - 1).x, segments.point(index + 1).x)
    }
}

/// Snap when `value` is already within `threshold` of a grid line.
fn snap_near(value: Smp, divisions: i32, threshold: Smp, min: Smp, max: Smp) -> Smp {
    if divisions <= 1 {
        return value;
    }

    let snapped = (value * divisions as Smp).round() / divisions as Smp;

    if (snapped - value).abs() < threshold {
        snapped.clamp(min, max)
    } else {
        value
    }
}

#[cfg(test)]
mod tests;
