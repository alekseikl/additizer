use egui::{
    Color32, CursorIcon, DragValue, FontFamily, FontId, LayerId, Mesh, Order, Painter,
    PointerButton, Pos2, Rect, Response, RichText, Sense, Shape, Stroke, StrokeKind, Ui, Vec2,
    ecolor::Hsva, layers::ShapeIdx, pos2, vec2,
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
/// Space between a point and its loop or release letter.
const MARKER_GAP: f32 = 3.0;

const HELP: &str = "Double-click to add or remove a point. Drag a midpoint to bend the segment. Hold Shift to mirror that bend onto the next segment. Right-click to flip.";
const HELP_MARKERS: &str = "Double-click to add or remove a point. Drag a midpoint to bend the segment. Hold Shift to mirror that bend onto the next segment. Right-click a point to set the loop or release.";
const SMOOTH_TIP: &str = "Ease this segment with a sine curve.";
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

/// Handle held by the pointer.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Drag {
    Point(usize),
    Power(usize),
}

/// Revision and plot size the cached curve samples were built from.
#[derive(Clone, Copy, PartialEq)]
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
            [0, 1].map(|channel| {
                let phase = phase[channel];
                (phase, segments.value_at(phase, None, None))
            })
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
        let size = vec2(ui.available_width(), EDITOR_HEIGHT);
        let (plot, response) = ui.allocate_exact_size(size, Sense::click());

        if plot.is_positive() {
            self.plot(ui, plot, &response, segments, playhead);
        }

        ui.add_space(8.0);
        self.toolbar(ui, segments);
        ui.add_space(4.0);
        ui.label(RichText::new(self.help()).small().weak());

        segments.revision() != revision
    }

    fn help(&self) -> &'static str {
        if self.markers { HELP_MARKERS } else { HELP }
    }

    /// Handles edit the curve first. The curve is then painted into a slot reserved
    /// before them, so it stays behind the handles and still matches this frame.
    fn plot(
        &mut self,
        ui: &Ui,
        plot: Rect,
        response: &Response,
        segments: &mut Segments,
        playhead: Option<[(Smp, Smp); 2]>,
    ) {
        let curve_slot = ui.painter().with_clip_rect(plot).add(Shape::Noop);

        Self::canvas(response, plot, segments);

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

        if let Some(playhead) = playhead {
            self.paint_playheads(ui.painter(), plot, playhead);
        }

        self.paint_curve_behind(ui, plot, segments, curve_slot);

        if let Some(index) = remove {
            segments.remove_point(index);
        }
    }

    /// Empty plot: double-click inserts a point, right-click flips the curve.
    fn canvas(response: &Response, plot: Rect, segments: &mut Segments) {
        Self::canvas_menu(response, segments);

        if response.double_clicked_by(PointerButton::Primary)
            && let Some(pos) = response.interact_pointer_pos().or(response.hover_pos())
        {
            Self::add_point(segments, Self::curve_pos(plot, pos));
        }
    }

    /// One bend handle: drag, double-click, menu, and paint.
    fn bend(&mut self, ui: &Ui, plot: Rect, segments: &mut Segments, index: usize) {
        if !self.power_active(plot, segments, index) {
            if self.drag == Some(Drag::Power(index)) {
                self.end_drag();
            }

            return;
        }

        let center = Self::screen_pos(plot, self.power_position(segments, index));
        let response = Self::handle_response(
            ui,
            plot,
            center,
            POWER_GRAB_RADIUS,
            ("segments-bend", index),
        );

        Self::power_menu(&response, segments, index);

        if response.double_clicked_by(PointerButton::Primary) {
            if Self::shift_down(ui) {
                segments.reset_powers();
            } else {
                segments.set_power(index, 0.0);
            }

            self.end_drag();
        } else {
            self.bend_drag(ui, plot, segments, index, &response);
        }

        // The edit above may have moved the handle.
        let center = Self::screen_pos(plot, self.power_position(segments, index));
        let dragging = self.drag == Some(Drag::Power(index));

        ui.painter()
            .circle_filled(center, POWER_RADIUS, self.power_color);
        Self::paint_grab(ui, &response, center, POWER_GRAB_RADIUS, dragging);
    }

    /// One breakpoint: drag, double-click, menu, and paint.
    ///
    /// `true` asks the caller to remove this point after the frame is painted.
    fn point(&mut self, ui: &Ui, plot: Rect, segments: &mut Segments, index: usize) -> bool {
        let center = Self::screen_pos(plot, Self::curve_point(segments.point(index)));
        let response =
            Self::handle_response(ui, plot, center, GRAB_RADIUS, ("segments-point", index));
        let mut remove = self.point_menu(&response, segments, index);

        if response.double_clicked_by(PointerButton::Primary) {
            remove |= Self::is_inner(segments, index);
            self.end_drag();
        } else {
            self.point_drag(ui, plot, segments, index, &response);
        }

        if remove {
            self.end_drag();
        }

        self.paint_point(ui, plot, segments, index, &response);
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
            let pointer = Self::curve_pos(plot, pos);

            self.grab = Some(CircleGrab::new(
                pointer,
                Self::curve_point(segments.point(index)),
            ));
            self.drag = Some(Drag::Point(index));
        }

        let Some(pos) = self.drag_pos(ui, Drag::Point(index)) else {
            return;
        };
        let Some(grab) = self.grab else {
            return;
        };
        let placed = grab.place(Self::curve_pos(plot, pos));

        if placed != grab.center() {
            self.move_point(segments, index, placed.x, placed.y, !Self::shift_down(ui));
        }
    }

    /// Bend follows vertical pointer travel. Shift mirrors the bend onto the next segment.
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
        let y = Self::curve_pos(plot, pos).y;

        if let Some(last) = self.power_y
            && y != last
        {
            self.move_power(segments, index, y - last, Self::shift_down(ui));
        }

        self.power_y = Some(y);
    }

    /// Pointer once this press is a drag and no other handle is held.
    ///
    /// While it can still be a click, a repaint is requested so the hold can elapse.
    fn drag_begin(&self, ui: &Ui, response: &Response) -> Option<Pos2> {
        if self.drag.is_some() || !response.is_pointer_button_down_on() {
            return None;
        }

        let (origin, elapsed) = ui.input(|input| {
            let elapsed = input
                .pointer
                .press_start_time()
                .map_or(0.0, |start| input.time - start);

            (input.pointer.press_origin(), elapsed)
        });
        let origin = origin?;
        let pointer = response.interact_pointer_pos().or(response.hover_pos());
        let moved = pointer.is_some_and(|pos| pos.distance(origin) > MAX_CLICK_DIST);

        if (moved || elapsed > MAX_CLICK_DURATION)
            && let Some(pos) = pointer
        {
            return Some(pos);
        }

        let remaining = (MAX_CLICK_DURATION - elapsed).max(0.0) as f32;

        ui.ctx().request_repaint_after_secs(remaining);
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

    fn end_drag(&mut self) {
        self.drag = None;
        self.grab = None;
        self.power_y = None;
    }

    /// Clickable square around `center`, kept inside the plot.
    fn handle_response(
        ui: &Ui,
        plot: Rect,
        center: Pos2,
        radius: f32,
        id: impl std::hash::Hash + std::fmt::Debug,
    ) -> Response {
        let rect = Rect::from_center_size(center, Vec2::splat(radius * 2.0)).intersect(plot);
        let rect = if rect.is_positive() {
            rect
        } else {
            Rect::from_center_size(center, Vec2::splat(1.0))
        };

        ui.interact(rect, ui.make_persistent_id(id), Sense::click_and_drag())
    }

    /// Insert a point at `point`, with the easing of the segment it splits.
    fn add_point(segments: &mut Segments, point: Pos2) {
        let index = Self::insertion_index(segments, point.x);
        let smooth = segments.point(index.saturating_sub(1)).smooth;

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

        let (min_x, max_x) = Self::x_limits(segments, index);
        let mut x = x.clamp(min_x, max_x);
        let mut y = y.clamp(0.0, 1.0);

        if snap {
            x = Self::snap_near(x, self.grid_x, SNAP_X, min_x, max_x);
            y = Self::snap_near(y, self.grid_y, SNAP_Y, 0.0, 1.0);
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

    /// Bend segment `index` by a vertical pointer move. `sync_next` mirrors the bend
    /// onto the next segment.
    fn move_power(&self, segments: &mut Segments, index: usize, delta_y: Smp, sync_next: bool) {
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

        let Some(next) = self.segment_end_idx(segments, index) else {
            return;
        };

        if self.segment_end(segments, next).is_some() && segments.power(next) != -power {
            segments.set_power(next, -power);
        }
    }

    /// `true` when the menu asked to remove this point. Other choices edit `segments` directly.
    fn point_menu(&self, response: &Response, segments: &mut Segments, index: usize) -> bool {
        let mut remove = false;

        response.context_menu(|ui| {
            if index >= segments.num_points() {
                return;
            }

            Self::smooth_item(ui, segments, index);

            if self.markers {
                Self::marker_items(ui, segments, index);
            }

            if Self::is_inner(segments, index) && ui.button("Remove point").clicked() {
                remove = true;
                ui.close();
            }
        });

        remove
    }

    fn power_menu(response: &Response, segments: &mut Segments, index: usize) {
        response.context_menu(|ui| {
            if index < segments.num_points() {
                Self::smooth_item(ui, segments, index);
            }

            if ui.button("Reset bend").clicked() {
                segments.set_power(index, 0.0);
                ui.close();
            }
        });
    }

    fn canvas_menu(response: &Response, segments: &mut Segments) {
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

    /// Toggles the sine easing of segment `index`.
    fn smooth_item(ui: &mut Ui, segments: &mut Segments, index: usize) {
        let smooth = segments.point(index).smooth;
        let label = if smooth { "Flat" } else { "Smooth" };

        if ui.button(label).on_hover_text(SMOOTH_TIP).clicked() {
            segments.set_smooth(index, !smooth);
            ui.close();
        }
    }

    /// Loop and release choices for point `index`.
    fn marker_items(ui: &mut Ui, segments: &mut Segments, index: usize) {
        if segments.loop_point_idx() == Some(index) {
            if ui.button("Clear loop").clicked() {
                segments.set_loop_point(None);
                ui.close();
            }
        } else {
            let can_loop = index + 1 < segments.num_points()
                && segments
                    .release_point_idx()
                    .is_none_or(|release| index < release);

            if can_loop && ui.button("Loop point").on_hover_text(LOOP_TIP).clicked() {
                segments.set_loop_point(Some(index));
                ui.close();
            }
        }

        if segments.release_point_idx() == Some(index) {
            if ui.button("Clear release").clicked() {
                segments.set_release_point(None);
                ui.close();
            }
        } else {
            let can_release = segments
                .loop_point_idx()
                .is_none_or(|loop_point| index > loop_point);

            if can_release
                && ui
                    .button("Release point")
                    .on_hover_text(RELEASE_TIP)
                    .clicked()
            {
                segments.set_release_point(Some(index));
                ui.close();
            }
        }
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

    /// A bend handle needs a sloped segment that is wide enough to show it.
    fn power_active(&self, plot: Rect, segments: &Segments, index: usize) -> bool {
        let Some(end) = self.segment_end(segments, index) else {
            return false;
        };
        let from = segments.point(index);

        plot.width() * (end.x - from.x) >= MIN_POWER_WIDTH && end.y != from.y
    }

    /// Breakpoint that ends segment `index`. The last segment of a closed curve wraps
    /// to the start, so this is also the next segment to bend.
    fn segment_end_idx(&self, segments: &Segments, index: usize) -> Option<usize> {
        let n = segments.num_points();

        if index + 1 < n {
            Some(index + 1)
        } else if self.closed && index < n {
            Some(0)
        } else {
            None
        }
    }

    /// End of segment `index` in curve space. The closing segment ends one cycle past the start.
    fn segment_end(&self, segments: &Segments, index: usize) -> Option<Pos2> {
        let end = self.segment_end_idx(segments, index)?;
        let point = segments.point(end);
        let wrap = if end == 0 { 1.0 } else { 0.0 };

        Some(pos2(point.x + wrap, point.y))
    }

    /// Midpoint in phase, power-scaled in level. A wrap past 1 folds back into the plot.
    fn power_position(&self, segments: &Segments, index: usize) -> Pos2 {
        let from = Self::curve_point(segments.point(index));
        let Some(end) = self.segment_end(segments, index) else {
            return from;
        };
        let x = (from.x + end.x) * 0.5;
        let x = if x >= 1.0 { x - 1.0 } else { x };
        let t = power_scale(0.5, segments.power(index));

        pos2(x, from.y + t * (end.y - from.y))
    }

    /// Polyline with one sample per horizontal pixel and every breakpoint kept.
    ///
    /// `render_points` runs again when [`Segments::revision`] or the plot size changes.
    /// The polyline is projected into `plot` on every call.
    fn curve_points(&mut self, plot: Rect, segments: &Segments) {
        if plot.width() <= 0.0 {
            self.points.clear();
            self.curve_cache = None;
            return;
        }

        let cache = CurveCache {
            revision: segments.revision(),
            size: plot.size(),
        };

        if self.curve_cache != Some(cache) {
            let steps = cache.size.x.round().max(1.0) as usize;

            segments.render_points(steps, &mut self.curve, &mut self.scratch);
            self.curve_cache = Some(cache);
        }

        self.points.clear();
        self.points.extend(
            self.curve
                .iter()
                .map(|&[x, y]| Self::screen_pos(plot, pos2(x, y))),
        );
    }

    /// Move the curve shapes into `slot`, which was reserved before the handles.
    fn paint_curve_behind(&mut self, ui: &Ui, plot: Rect, segments: &Segments, slot: ShapeIdx) {
        let layer_id = ui.layer_id();
        let clip = ui.painter().with_clip_rect(plot).clip_rect();
        let first = ui
            .ctx()
            .graphics(|gfx| gfx.get(layer_id).map_or(0, |list| list.all_entries().len()));

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

        Self::paint_plot(&painter, plot, background);
        self.curve_points(plot, segments);
        self.paint_fill(&painter, plot, background);

        if let Some(region) = self.loop_region(plot, segments) {
            let color = Color32::from(LOOP_FILL_COLOR);
            Self::paint_gradient(&painter, region, color, color.a(), color.a());
        }

        self.paint_grid(&painter, plot);
        self.paint_curve(&painter);
    }

    fn paint_plot(painter: &Painter, plot: Rect, background: Color32) {
        painter.rect_filled(plot, 2.0, background);
        painter.rect_stroke(
            plot,
            2.0,
            Stroke::new(1.0, Color32::from_white_alpha(32)),
            StrokeKind::Inside,
        );
    }

    fn paint_grid(&self, painter: &Painter, plot: Rect) {
        let stroke = Stroke::new(1.0, GRID_COLOR);

        for i in 1..self.grid_x {
            let x = plot.left() + plot.width() * (i as f32 / self.grid_x as f32);
            painter.line_segment([pos2(x, plot.top()), pos2(x, plot.bottom())], stroke);
        }

        for i in 1..self.grid_y {
            let y = plot.bottom() - plot.height() * (i as f32 / self.grid_y as f32);
            painter.line_segment([pos2(plot.left(), y), pos2(plot.right(), y)], stroke);
        }
    }

    fn paint_curve(&self, painter: &Painter) {
        if self.points.len() >= 2 {
            painter.line(self.points.clone(), Stroke::new(2.0, self.wave_color));
        }
    }

    /// Gradient under the curve.
    ///
    /// The whole plot gets one vertical gradient, and an opaque cover hides it above
    /// the polyline. One quad keeps the gradient smooth.
    fn paint_fill(&self, painter: &Painter, plot: Rect, background: Color32) {
        Self::paint_gradient(
            painter,
            plot,
            self.wave_color,
            self.fill_top_alpha,
            self.fill_bottom_alpha,
        );
        Self::paint_cover(painter, plot.top(), &self.points, background);
    }

    /// `color` fading from `top_alpha` to `bottom_alpha` across `rect`.
    fn paint_gradient(
        painter: &Painter,
        rect: Rect,
        color: Color32,
        top_alpha: u8,
        bottom_alpha: u8,
    ) {
        if !rect.is_positive() {
            return;
        }

        let top = Self::with_alpha(color, top_alpha);
        let bottom = Self::with_alpha(color, bottom_alpha);
        let mut mesh = Mesh::default();

        Self::add_quad(
            &mut mesh,
            [
                (rect.left_top(), top),
                (rect.right_top(), top),
                (rect.right_bottom(), bottom),
                (rect.left_bottom(), bottom),
            ],
        );
        painter.add(Shape::mesh(mesh));
    }

    /// Opaque background between `top` and the polyline.
    fn paint_cover(painter: &Painter, top: f32, points: &[Pos2], background: Color32) {
        let cover = Color32::from_rgb(background.r(), background.g(), background.b());
        let mut mesh = Mesh::default();

        for window in points.windows(2) {
            let (start, end) = (window[0], window[1]);

            if end.x > start.x {
                Self::add_quad(
                    &mut mesh,
                    [
                        (pos2(start.x, top), cover),
                        (pos2(end.x, top), cover),
                        (end, cover),
                        (start, cover),
                    ],
                );
            }
        }

        if !mesh.is_empty() {
            painter.add(Shape::mesh(mesh));
        }
    }

    /// Screen span of the loop, from the loop point to the release point or the end.
    fn loop_region(&self, plot: Rect, segments: &Segments) -> Option<Rect> {
        if !self.markers {
            return None;
        }

        let start = segments.loop_point()?;
        let end_x = segments.release_point().map_or(1.0, |point| point.x);
        let left = Self::screen_x(plot, start.x);
        let right = Self::screen_x(plot, end_x);

        (right > left).then(|| Rect::from_x_y_ranges(left..=right, plot.y_range()))
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
        let center = Self::screen_pos(plot, Self::curve_point(point));
        let marker = self.marker(segments, index);
        let color = marker.map_or(POINT_COLOR, |(_, color)| color);
        let painter = ui.painter();

        painter.circle_filled(center, POINT_RADIUS, color);

        if let Some((label, color)) = marker {
            Self::paint_marker(painter, plot, center, label, color);
        }

        let dragging = self.drag == Some(Drag::Point(index));
        Self::paint_grab(ui, response, center, GRAB_RADIUS, dragging);

        if dragging {
            let level = Units::Normalized.format(point.y.clamp(0.0, 1.0));
            Self::paint_level(ui, center, level);
        }
    }

    /// Letter and color of the loop or release point. Never both: the loop sits before the release.
    fn marker(&self, segments: &Segments, index: usize) -> Option<(&'static str, Color32)> {
        if !self.markers {
            None
        } else if segments.loop_point_idx() == Some(index) {
            Some(("L", LOOP_COLOR))
        } else if segments.release_point_idx() == Some(index) {
            Some(("R", RELEASE_COLOR))
        } else {
            None
        }
    }

    fn paint_marker(painter: &Painter, plot: Rect, center: Pos2, label: &str, color: Color32) {
        let galley = painter.layout_no_wrap(label.to_owned(), Self::bold(10.0), color);
        let x = center.x - galley.size().x * 0.5;
        let y = Self::stack_y(
            center.y,
            POINT_RADIUS + MARKER_GAP,
            galley.size().y,
            plot.top(),
        );

        painter.galley(pos2(x, y), galley, color);
    }

    /// Hover ring, and a filled disc while dragging. Drawn on the handle layer.
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

        if dragging {
            painter.circle_filled(center, radius, Color32::from_white_alpha(0x40));
        }

        painter.circle_stroke(center, radius, Stroke::new(1.0, Color32::WHITE));
    }

    /// Level readout next to the dragged point, kept inside the clip rect.
    fn paint_level(ui: &Ui, anchor: Pos2, text: String) {
        let painter = ui.painter().clone().with_layer_id(Self::handle_layer(ui));
        let galley = painter.layout_no_wrap(text, Self::bold(11.0), LABEL_TEXT_COLOR.into());
        let padding = vec2(LABEL_PADDING, LABEL_PADDING * 0.5);
        let size = galley.size() + padding * 2.0;
        let bounds = ui.clip_rect();
        let top = Self::stack_y(
            anchor.y,
            GRAB_RADIUS + LABEL_POINTER_GAP,
            size.y,
            bounds.top(),
        );
        let min = pos2(
            (anchor.x - size.x * 0.5)
                .clamp(bounds.left(), (bounds.right() - size.x).max(bounds.left())),
            top.clamp(bounds.top(), (bounds.bottom() - size.y).max(bounds.top())),
        );
        let rect = Rect::from_min_size(min, size);

        painter.rect(
            rect,
            LABEL_CORNER_RADIUS,
            LABEL_BG_COLOR,
            Stroke::new(1.0, LABEL_BORDER_COLOR),
            StrokeKind::Inside,
        );
        painter.galley(rect.min + padding, galley, LABEL_TEXT_COLOR.into());
    }

    fn handle_layer(ui: &Ui) -> LayerId {
        LayerId::new(Order::Foreground, ui.id().with("segments-handle"))
    }

    /// Left stays the curve color. The right channel keeps that saturation and value.
    fn paint_playheads(&self, painter: &Painter, plot: Rect, playhead: [(Smp, Smp); 2]) {
        let [left, right] = playhead.map(|(phase, level)| Self::playhead_pos(plot, phase, level));

        if left.distance_sq(right) > PLAYHEAD_OVERLAP_SQ {
            let mut color = Hsva::from(self.wave_color);
            color.h = RIGHT_PLAYHEAD_HUE;

            painter.circle_filled(right, PLAYHEAD_RADIUS, color);
        }

        painter.circle_filled(left, PLAYHEAD_RADIUS, self.wave_color);
    }

    /// Screen position of the playhead at `(phase, level)`.
    fn playhead_pos(plot: Rect, phase: Smp, level: Smp) -> Pos2 {
        let phase = sanitize(phase, 0.0, 1.0);
        let level = sanitize(level, 0.0, 1.0);

        Self::screen_pos(plot, pos2(phase, level))
    }

    /// Pointer position while a drag is held, including outside the plot and the window.
    fn drag_pointer(ui: &Ui) -> Option<Pos2> {
        let pos = ui.input(|input| input.pointer.latest_pos())?;
        let from_global = ui.ctx().layer_transform_from_global(ui.layer_id());

        Some(from_global.map_or(pos, |transform| transform * pos))
    }

    /// Plot position of a screen point. Values outside 0–1 are past the canvas.
    fn curve_pos(plot: Rect, pos: Pos2) -> Pos2 {
        pos2(
            (pos.x - plot.left()) / plot.width(),
            (plot.bottom() - pos.y) / plot.height(),
        )
    }

    /// Screen position of a plot point. Values outside 0–1 land on the plot edge.
    fn screen_pos(plot: Rect, point: Pos2) -> Pos2 {
        pos2(
            Self::screen_x(plot, point.x),
            plot.bottom() - point.y.clamp(0.0, 1.0) * plot.height(),
        )
    }

    fn screen_x(plot: Rect, x: Smp) -> f32 {
        plot.left() + x.clamp(0.0, 1.0) * plot.width()
    }

    /// Curve-space position of a breakpoint.
    fn curve_point(point: SegPoint) -> Pos2 {
        pos2(point.x, point.y)
    }

    /// Interior points can be removed. The ends stay.
    fn is_inner(segments: &Segments, index: usize) -> bool {
        index > 0 && index + 1 < segments.num_points()
    }

    fn shift_down(ui: &Ui) -> bool {
        ui.input(|input| input.modifiers.shift)
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

    /// Top of a box `height` tall placed `offset` above `anchor_y`, or `offset` below it
    /// when the box would pass `top`.
    fn stack_y(anchor_y: f32, offset: f32, height: f32, top: f32) -> f32 {
        let above = anchor_y - offset - height;

        if above >= top {
            above
        } else {
            anchor_y + offset
        }
    }

    fn bold(size: f32) -> FontId {
        FontId::new(size, FontFamily::Name("Bold".into()))
    }

    fn with_alpha(color: Color32, alpha: u8) -> Color32 {
        Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha)
    }

    /// Two triangles over `corners`, given in winding order.
    fn add_quad(mesh: &mut Mesh, corners: [(Pos2, Color32); 4]) {
        let index = mesh.vertices.len() as u32;

        for (pos, color) in corners {
            mesh.colored_vertex(pos, color);
        }

        mesh.add_triangle(index, index + 1, index + 2);
        mesh.add_triangle(index, index + 2, index + 3);
    }
}

#[cfg(test)]
mod tests;
