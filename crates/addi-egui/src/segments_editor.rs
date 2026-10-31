use egui::{
    Color32, ColorImage, Context, CursorIcon, DragValue, FontFamily, FontId, LayerId, Mesh, Order,
    Painter, PointerButton, Pos2, Rect, Response, RichText, Sense, Shape, Stroke, StrokeKind,
    TextureHandle, TextureId, TextureOptions, Ui, Vec2, ecolor::Hsva, epaint::Vertex, pos2, vec2,
};

use addi_dsp::{
    power_scale,
    segments::{MAX_POINTS, MAX_POWER, SegPoint, Segments},
    utils::sanitize,
};
use addi_engine::{Smp, StereoSample};

use crate::{units::Units, utils::hsva};

use utils::{Drag, Handle};

mod utils;

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
const LOOP_FILL_COLOR: Hsva = hsva(0.0, 0.9092, 1.0, 0.05);
const RELEASE_COLOR: Color32 = Color32::from_rgb(0xff, 0x7a, 0x59);
const FILL_TOP_ALPHA: u8 = 0x47;
const FILL_BOTTOM_ALPHA: u8 = 0x28;
const ENVELOPE_WAVE: Hsva = hsva(0.3374, 0.9, 0.2, 1.0);
const ENVELOPE_POWER: Hsva = hsva(0.348, 0.7, 0.4, 1.0);
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
const HELP_PAINT: &str = "Drag to paint this shape into each grid column, up to the pointer. Hold Shift to ignore the vertical grid. Hold Cmd or Ctrl to edit points.";
const SMOOTH_TIP: &str = "Ease this segment with a sine curve.";
const LOOP_TIP: &str = "Repeat from here. With no release point the loop runs to the end. With one, it runs until release while the key is held.";
const RELEASE_TIP: &str = "Sustain this level while the key is held. On release, continue from here starting at the current level.";

const TOOL_BUTTON: Vec2 = Vec2::new(30.0, 22.0);
const PAINT_COLUMN_ALPHA: u8 = 36;
const EDIT_ICON: [(f32, f32); 3] = [(0.08, 0.28), (0.42, 0.82), (0.92, 0.38)];

/// Point editing, or one paint shape. Clicking the active shape returns to points.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Tool {
    Edit,
    Paint(Pattern),
}

const TOOLS: [Tool; 6] = [
    Tool::Edit,
    Tool::Paint(Pattern::Step),
    Tool::Paint(Pattern::Half),
    Tool::Paint(Pattern::Down),
    Tool::Paint(Pattern::Up),
    Tool::Paint(Pattern::Tri),
];

/// Shape stamped into one grid column. High samples sit at the pointer; low samples sit on the bottom.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Pattern {
    Step,
    Half,
    Down,
    Up,
    Tri,
}

impl Tool {
    fn tip(self) -> &'static str {
        match self {
            Self::Edit => "Edit points",
            Self::Paint(pattern) => pattern.tip(),
        }
    }
}

impl Pattern {
    /// `(along the column, height)`. Height `1` follows the pointer.
    fn shape(self) -> &'static [(Smp, Smp)] {
        match self {
            Self::Step => &[(0.0, 1.0), (1.0, 1.0)],
            Self::Half => &[(0.0, 1.0), (0.5, 1.0), (0.5, 0.0), (1.0, 0.0)],
            Self::Down => &[(0.0, 1.0), (1.0, 0.0)],
            Self::Up => &[(0.0, 0.0), (1.0, 1.0)],
            Self::Tri => &[(0.0, 0.0), (0.5, 1.0), (1.0, 0.0)],
        }
    }

    fn scaled(self, from_x: Smp, to_x: Smp, level: Smp) -> Vec<SegPoint> {
        self.shape()
            .iter()
            .map(|&(t, height)| SegPoint {
                x: from_x + t * (to_x - from_x),
                y: height * level,
                ..SegPoint::default()
            })
            .collect()
    }

    fn tip(self) -> &'static str {
        match self {
            Self::Step => "Paint a step",
            Self::Half => "Paint a half step",
            Self::Down => "Paint a falling ramp",
            Self::Up => "Paint a rising ramp",
            Self::Tri => "Paint a triangle",
        }
    }
}

pub struct SegmentsEditor {
    grid_x: i32,
    grid_y: i32,
    /// Handle being dragged. The center keeps its offset from the pointer.
    drag: Option<Drag>,
    /// Power of the bent segment when its drag started.
    drag_power: Smp,
    /// Curve samples in 0–1 space.
    curve: Vec<[Smp; 2]>,
    /// Workspace for [`Segments::render_points`].
    scratch: Vec<Smp>,
    /// Cached curve samples projected into the current plot.
    points: Vec<Pos2>,
    /// Revision and plot size the cached curve samples were built from.
    curve_cache: Option<(u64, Vec2)>,
    /// Endpoints share one level, and the last bend wraps back to the start.
    wrapped: bool,
    /// Loop and release markers. The segments envelope turns this on.
    markers: bool,
    /// Selected edit tool.
    tool: Tool,
    /// Shape used when the command key temporarily turns paint on.
    pattern: Pattern,
    /// Grid column under the pointer while painting.
    paint_column: Option<i32>,
    wave_color: Color32,
    power_color: Color32,
    /// Plot gradient, uploaded on first paint.
    fill: Option<TextureHandle>,
}

impl Default for SegmentsEditor {
    fn default() -> Self {
        Self {
            grid_x: 8,
            grid_y: 1,
            drag: None,
            drag_power: 0.0,
            curve: Vec::new(),
            scratch: Vec::new(),
            points: Vec::new(),
            curve_cache: None,
            wrapped: true,
            markers: false,
            tool: Tool::Edit,
            pattern: Pattern::Down,
            paint_column: None,
            wave_color: WAVE_COLOR,
            power_color: POWER_COLOR,
            fill: None,
        }
    }
}

impl SegmentsEditor {
    /// Endpoints keep their own levels.
    pub fn lookup() -> Self {
        Self {
            wrapped: false,
            ..Self::default()
        }
    }

    /// Curve, fill, and playhead match the envelope module.
    pub fn envelope() -> Self {
        Self {
            wrapped: false,
            markers: true,
            wave_color: Color32::from(ENVELOPE_WAVE),
            power_color: Color32::from(ENVELOPE_POWER),
            ..Self::default()
        }
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
    pub fn ui_at(
        &mut self,
        ui: &mut Ui,
        segments: &mut Segments,
        playhead: Option<[(Smp, Smp); 2]>,
    ) -> bool {
        let revision = segments.revision();
        let size = vec2(ui.available_width(), EDITOR_HEIGHT);
        let (plot, response) = ui.allocate_exact_size(size, Sense::click_and_drag());

        if plot.is_positive() {
            self.plot(ui, plot, &response, segments, playhead);
        }

        ui.add_space(8.0);
        self.toolbar(ui);
        ui.add_space(4.0);
        ui.label(RichText::new(self.help()).small().weak());

        segments.revision() != revision
    }

    fn help(&self) -> &'static str {
        if matches!(self.tool, Tool::Paint(_)) {
            HELP_PAINT
        } else if self.markers {
            HELP_MARKERS
        } else {
            HELP
        }
    }

    /// Paint is selected, or point editing is selected and the command key is held.
    /// The command key swaps the two.
    fn painting(&self, ui: &Ui) -> bool {
        let command = ui.input(|input| input.modifiers.command);

        matches!(self.tool, Tool::Paint(_)) != command
    }

    fn plot(
        &mut self,
        ui: &Ui,
        plot: Rect,
        response: &Response,
        segments: &mut Segments,
        playhead: Option<[(Smp, Smp); 2]>,
    ) {
        let mut remove = None;
        let painting = self.painting(ui);

        if !ui.input(|input| input.pointer.primary_down()) {
            self.drag = None;
        }

        if painting {
            self.drag = None;
            self.paint_pointer(ui, plot, response, segments);
            Self::canvas_menu(response, segments);
        } else {
            self.paint_column = None;
            Self::interact_canvas(response, plot, segments);

            for index in 0..segments.num_points() {
                if self.interact_point(ui, plot, segments, index) {
                    remove = Some(index);
                }

                self.interact_power_point(ui, plot, segments, index);
            }
        }

        self.paint_scene(ui, plot, segments);

        if painting && (response.hovered() || response.is_pointer_button_down_on()) {
            ui.ctx().set_cursor_icon(CursorIcon::Crosshair);
        }

        if let Some(playhead) = playhead {
            self.paint_playheads(ui.painter(), plot, playhead);
        }

        if let Some(index) = remove {
            segments.remove_point(index);
        }
    }

    fn interact_canvas(response: &Response, plot: Rect, segments: &mut Segments) {
        Self::canvas_menu(response, segments);

        if response.double_clicked_by(PointerButton::Primary)
            && let Some(pos) = response.interact_pointer_pos().or(response.hover_pos())
        {
            Self::add_point(segments, Self::curve_pos(plot, pos));
        }
    }

    /// One power point: drag, double-click, and menu.
    fn interact_power_point(&mut self, ui: &Ui, plot: Rect, segments: &mut Segments, index: usize) {
        if !self.power_point_active(plot, segments, index) {
            return;
        }

        let center = Self::screen_pos(plot, self.power_position(segments, index));
        let response = Self::handle_response(
            ui,
            plot,
            center,
            POWER_GRAB_RADIUS,
            ("segments-power", index),
        );

        Self::power_menu(&response, segments, index);

        if response.double_clicked_by(PointerButton::Primary) {
            segments.set_power(index, 0.0);
            self.drag = None;
        } else if let Some(pos) = self.drag_begin(ui, &response) {
            self.drag = Some(Drag::new(
                Self::curve_pos(plot, pos),
                self.power_position(segments, index),
                Handle::Power(index),
            ));
            self.drag_power = segments.power(index);
        }

        if let Some((_, delta)) = self.drag_move(ui, plot, Handle::Power(index)) {
            self.move_power(ui, segments, index, delta.y);
        }
    }

    /// One breakpoint: drag, double-click, and menu.
    fn interact_point(
        &mut self,
        ui: &Ui,
        plot: Rect,
        segments: &mut Segments,
        index: usize,
    ) -> bool {
        let center = Self::screen_pos(plot, Self::curve_point(segments.point(index)));
        let response =
            Self::handle_response(ui, plot, center, GRAB_RADIUS, ("segments-point", index));
        let mut remove = self.point_menu(&response, segments, index);

        if response.double_clicked_by(PointerButton::Primary) {
            remove |= Self::is_inner(segments, index);
        } else if let Some(pos) = self.drag_begin(ui, &response) {
            self.drag = Some(Drag::new(
                Self::curve_pos(plot, pos),
                Self::curve_point(segments.point(index)),
                Handle::Point(index),
            ));
        }

        if let Some((placed, _)) = self.drag_move(ui, plot, Handle::Point(index)) {
            self.move_point(ui, segments, index, placed.x, placed.y);
        }

        if remove {
            self.drag = None;
        }

        remove
    }

    /// Pointer once this press is a drag and no other handle is held.
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

    /// Advances `handle` while the primary button is held.
    fn drag_move(&mut self, ui: &Ui, plot: Rect, handle: Handle) -> Option<(Pos2, Vec2)> {
        if self.dragged() != Some(handle) {
            return None;
        }

        let pos = ui.input(|input| input.pointer.latest_pos())?;
        let drag = self.drag.as_mut()?;

        Some(drag.move_pos(Self::curve_pos(plot, pos)))
    }

    fn dragged(&self) -> Option<Handle> {
        self.drag.map(Drag::handle)
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

    /// Shift places the point freely.
    fn move_point(&self, ui: &Ui, segments: &mut Segments, index: usize, x: Smp, y: Smp) {
        if index >= segments.num_points() {
            return;
        }

        let (min_x, max_x) = Self::x_limits(segments, index);
        let mut x = x.clamp(min_x, max_x);
        let mut y = y.clamp(0.0, 1.0);

        if !Self::shift_down(ui) {
            x = Self::snap_near(x, self.grid_x, SNAP_X, min_x, max_x);
            y = Self::snap_near(y, self.grid_y, SNAP_Y, 0.0, 1.0);
        }

        self.match_wrapped_ends(segments, index, y);

        let current = segments.point(index);
        segments.set_point(index, SegPoint { x, y, ..current });
    }

    /// Wrapped curves keep the first and last levels equal.
    fn match_wrapped_ends(&self, segments: &mut Segments, index: usize, y: Smp) {
        if !self.wrapped {
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

    /// Bend segment `index` from [`Self::drag_power`] by the vertical offset `delta_y`.
    /// Shift mirrors the bend onto the next segment.
    fn move_power(&self, ui: &Ui, segments: &mut Segments, index: usize, delta_y: Smp) {
        let Some(end) = self.segment_end(segments, index) else {
            return;
        };
        let from = segments.point(index);

        if from.y == end.y {
            return;
        }

        let amount = POWER_MOUSE_MULTIPLIER * if from.y < end.y { -delta_y } else { delta_y };
        let power = sanitize(self.drag_power + amount, -MAX_POWER, MAX_POWER);

        if segments.power(index) != power {
            segments.set_power(index, power);
        }

        if !Self::shift_down(ui) {
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

            if ui.button("Reset bends").clicked() {
                segments.reset_powers();
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

    fn toolbar(&mut self, ui: &mut Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.label("Grid")
                .on_hover_text("Snap points while dragging. Hold Shift to place them freely.");
            ui.add(DragValue::new(&mut self.grid_x).range(1..=32));
            ui.label("×");
            ui.add(DragValue::new(&mut self.grid_y).range(1..=24));

            ui.separator();
            ui.spacing_mut().item_spacing.x = 2.0;
            ui.add_space(6.0);

            for tool in TOOLS {
                if self.tool_button(ui, tool).clicked() {
                    self.choose(tool);
                }
            }
        });
    }

    fn choose(&mut self, tool: Tool) {
        if let Tool::Paint(pattern) = tool {
            self.pattern = pattern;
        }

        self.tool = if self.tool == tool { Tool::Edit } else { tool };
    }

    fn tool_button(&self, ui: &mut Ui, tool: Tool) -> Response {
        let (rect, response) = ui.allocate_exact_size(TOOL_BUTTON, Sense::click());
        let selected = self.tool == tool;
        let visuals = ui.style().interact_selectable(&response, selected);

        ui.painter().rect(
            rect.expand(visuals.expansion),
            visuals.corner_radius,
            visuals.weak_bg_fill,
            visuals.bg_stroke,
            StrokeKind::Inside,
        );
        Self::paint_tool_icon(
            ui.painter(),
            rect.shrink(4.0),
            tool,
            visuals.fg_stroke.color,
        );

        response
            .on_hover_cursor(CursorIcon::PointingHand)
            .on_hover_text(tool.tip())
    }

    fn paint_tool_icon(painter: &Painter, icon: Rect, tool: Tool, color: Color32) {
        if !icon.is_positive() {
            return;
        }

        match tool {
            Tool::Edit => Self::paint_edit_icon(painter, icon, color),
            Tool::Paint(pattern) => Self::paint_pattern_icon(painter, icon, pattern, color),
        }
    }

    fn paint_edit_icon(painter: &Painter, icon: Rect, color: Color32) {
        let points = EDIT_ICON.map(|(x, y)| {
            pos2(
                icon.left() + x * icon.width(),
                icon.bottom() - y * icon.height(),
            )
        });

        painter.line(points.to_vec(), Stroke::new(1.0, color));

        for center in points {
            painter.circle_filled(center, 1.7, color);
        }
    }

    fn paint_pattern_icon(painter: &Painter, icon: Rect, pattern: Pattern, color: Color32) {
        let mut points = Vec::with_capacity(pattern.shape().len() + 2);
        let mut push = |pos: Pos2| {
            if points.last() != Some(&pos) {
                points.push(pos);
            }
        };

        push(pos2(icon.left(), icon.bottom()));

        for &(t, height) in pattern.shape() {
            push(pos2(
                icon.left() + t * icon.width(),
                icon.bottom() - height * icon.height(),
            ));
        }

        push(pos2(icon.right(), icon.bottom()));

        if points.len() >= 3 {
            painter.add(Shape::convex_polygon(
                points,
                Self::with_alpha(color, 0x55),
                Stroke::new(1.0, color),
            ));
        }
    }

    /// Highlights the column under the pointer and stamps into it while the button is held.
    fn paint_pointer(&mut self, ui: &Ui, plot: Rect, response: &Response, segments: &mut Segments) {
        let pos = response.interact_pointer_pos().or(response.hover_pos());

        self.paint_column = pos.map(|pos| self.column_at(plot, pos));

        let primary = ui.input(|input| input.pointer.button_down(PointerButton::Primary));

        if primary
            && response.is_pointer_button_down_on()
            && let Some(pos) = response.interact_pointer_pos()
        {
            self.paint_at(ui, plot, segments, pos);
        }
    }

    /// Shift places the level freely. Otherwise it snaps to the vertical grid.
    fn paint_at(&self, ui: &Ui, plot: Rect, segments: &mut Segments, pos: Pos2) {
        let mut level = Self::curve_pos(plot, pos).y.clamp(0.0, 1.0);

        if !Self::shift_down(ui) {
            level = Self::snap_near(level, self.grid_y, SNAP_Y, 0.0, 1.0);
        }

        let (from_x, to_x) = self.column_bounds(self.column_at(plot, pos));

        Self::paint_cell(segments, from_x, to_x, level, self.active_pattern());
    }

    fn active_pattern(&self) -> Pattern {
        match self.tool {
            Tool::Paint(pattern) => pattern,
            Tool::Edit => self.pattern,
        }
    }

    fn column_at(&self, plot: Rect, pos: Pos2) -> i32 {
        let columns = self.grid_x.max(1);
        let x = Self::curve_pos(plot, pos).x.clamp(0.0, 1.0);
        let column = (x * columns as Smp).floor() as i32;

        column.clamp(0, columns - 1)
    }

    fn column_bounds(&self, column: i32) -> (Smp, Smp) {
        let columns = self.grid_x.max(1) as Smp;
        let column = column.clamp(0, self.grid_x.max(1) - 1) as Smp;

        (column / columns, (column + 1.0) / columns)
    }

    /// Stamps `pattern` into `[from_x, to_x]`.
    ///
    /// The curve outside the column stays put. Each edge keeps the level the curve
    /// already had there, then steps vertically onto the shape. A stamp that would
    /// pass [`MAX_POINTS`] is left alone.
    fn paint_cell(segments: &mut Segments, from_x: Smp, to_x: Smp, level: Smp, pattern: Pattern) {
        if !(from_x >= 0.0 && from_x < to_x && to_x <= 1.0) {
            return;
        }

        let level = level.clamp(0.0, 1.0);
        let n = segments.num_points();
        let from_y = segments.value_at(from_x, None, None);
        let to_y = segments.value_at(to_x, None, None);
        let left = (0..n).find(|&index| segments.point(index).x == from_x);
        let right = (0..n).rev().find(|&index| segments.point(index).x == to_x);
        let pattern_points = pattern.scaled(from_x, to_x, level);
        let left_anchor = SegPoint {
            x: from_x,
            y: from_y,
            ..SegPoint::default()
        };
        let right_anchor = SegPoint {
            x: to_x,
            y: to_y,
            ..SegPoint::default()
        };
        let mut kept = Vec::with_capacity(n);

        for index in 0..n {
            if Self::keeps_point(segments, index, from_x, to_x, left, right) {
                kept.push(index);
            }
        }

        let split = kept
            .iter()
            .position(|&index| segments.point(index).x >= to_x)
            .unwrap_or(kept.len());
        let (before, after) = kept.split_at(split);
        let mut stamped = Vec::with_capacity(n + pattern_points.len() + 2);
        let mut map = vec![None; n];

        for (new_index, &old) in before.iter().enumerate() {
            map[old] = Some(new_index);
            stamped.push(segments.point(old));
        }

        if left.is_none() {
            stamped.push(left_anchor);
        }

        stamped.extend(pattern_points.iter().copied());

        if right.is_none() {
            stamped.push(right_anchor);
        }

        let after_at = stamped.len();

        for (offset, &old) in after.iter().enumerate() {
            map[old] = Some(after_at + offset);
            stamped.push(segments.point(old));
        }

        if stamped.len() > MAX_POINTS || segments.points() == stamped.as_slice() {
            return;
        }

        let loop_at = segments.loop_point_idx().and_then(|index| map[index]);
        let release_at = segments.release_point_idx().and_then(|index| map[index]);

        for index in (0..n).rev() {
            if map[index].is_none() {
                assert!(segments.remove_point(index));
            }
        }

        let mut at = before.len();

        if left.is_none() {
            assert!(segments.add_point(at, left_anchor));
            at += 1;
        }

        for point in pattern_points {
            assert!(segments.add_point(at, point));
            at += 1;
        }

        if right.is_none() {
            assert!(segments.add_point(at, right_anchor));
        }

        debug_assert_eq!(segments.points(), stamped.as_slice());

        segments.set_loop_point(None);
        segments.set_release_point(None);

        if let Some(index) = release_at {
            segments.set_release_point(Some(index));
        }

        if let Some(index) = loop_at {
            segments.set_loop_point(Some(index));
        }
    }

    /// Points outside the column stay. The first point on the left edge and the last
    /// point on the right edge stay too, so the stamp meets the old curve.
    fn keeps_point(
        segments: &Segments,
        index: usize,
        from_x: Smp,
        to_x: Smp,
        left: Option<usize>,
        right: Option<usize>,
    ) -> bool {
        let x = segments.point(index).x;

        if x < from_x || x > to_x {
            true
        } else if x == from_x {
            left == Some(index)
        } else if x == to_x {
            right == Some(index)
        } else {
            false
        }
    }

    /// A power point needs a sloped segment that is wide enough to show it.
    fn power_point_active(&self, plot: Rect, segments: &Segments, index: usize) -> bool {
        let Some(end) = self.segment_end(segments, index) else {
            return false;
        };
        let from = segments.point(index);

        plot.width() * (end.x - from.x) >= MIN_POWER_WIDTH && end.y != from.y
    }

    /// Breakpoint that ends segment `index`. The last segment of a wrapped curve wraps
    /// to the start, so this is also the next segment to bend.
    fn segment_end_idx(&self, segments: &Segments, index: usize) -> Option<usize> {
        let n = segments.num_points();

        if index + 1 < n {
            Some(index + 1)
        } else if self.wrapped && index < n {
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
    fn build_curve_points(&mut self, plot: Rect, segments: &Segments) {
        assert!(plot.width() > 0.0);

        let revision = segments.revision();
        let size = plot.size();

        if self.curve_cache != Some((revision, size)) {
            let steps = size.x.round().max(1.0) as usize;

            segments.render_points(steps, &mut self.curve, &mut self.scratch);
            self.curve_cache = Some((revision, size));
        }

        self.points.clear();
        self.points.extend(
            self.curve
                .iter()
                .map(|&[x, y]| Self::screen_pos(plot, pos2(x, y))),
        );

        assert!(self.points.len() >= 2);
    }

    fn paint_scene(&mut self, ui: &Ui, plot: Rect, segments: &Segments) {
        let painter = ui.painter().with_clip_rect(plot);

        self.build_curve_points(plot, segments);

        painter.rect_filled(plot, 2.0, ui.visuals().extreme_bg_color);
        painter.rect_stroke(
            plot,
            2.0,
            Stroke::new(1.0, Color32::from_white_alpha(32)),
            StrokeKind::Inside,
        );

        if let Some(region) = self.loop_region(plot, segments) {
            painter.rect_filled(region, 0.0, LOOP_FILL_COLOR);
        }

        self.paint_grid(&painter, plot);
        self.paint_fill(ui.ctx(), &painter, plot);
        self.paint_column_highlight(&painter, plot);
        painter.line(self.points.clone(), Stroke::new(2.0, self.wave_color));

        for index in 0..segments.num_points() {
            self.paint_point(ui, plot, segments, index);
            self.paint_power_point(ui, plot, segments, index);
        }
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

    fn paint_column_highlight(&self, painter: &Painter, plot: Rect) {
        let Some(column) = self.paint_column else {
            return;
        };
        let (from_x, to_x) = self.column_bounds(column);
        let left = Self::screen_x(plot, from_x);
        let right = Self::screen_x(plot, to_x);

        painter.rect_filled(
            Rect::from_x_y_ranges(left..=right, plot.y_range()),
            0.0,
            Self::with_alpha(self.wave_color, PAINT_COLUMN_ALPHA),
        );
    }

    /// Area under the polyline, shaded by a 1×2 gradient of the full plot.
    fn paint_fill(&mut self, ctx: &Context, painter: &Painter, plot: Rect) {
        assert!(self.points.len() >= 2 && plot.height() > 0.0);

        let mut mesh = Mesh::with_texture(self.fill_texture(ctx));
        let bottom = plot.bottom();

        for window in self.points.windows(2) {
            let (start, end) = (window[0], window[1]);

            if end.x > start.x {
                Self::add_fill_quad(&mut mesh, plot, bottom, start, end);
            }
        }

        if !mesh.is_empty() {
            painter.add(Shape::mesh(mesh));
        }
    }

    /// Quad down to `bottom`. V runs from the top texel center to the bottom one.
    fn add_fill_quad(mesh: &mut Mesh, plot: Rect, bottom: f32, start: Pos2, end: Pos2) {
        let index = mesh.vertices.len() as u32;
        let v = |y: f32| 0.25 + 0.5 * (y - plot.top()) / plot.height();

        for pos in [start, end, pos2(end.x, bottom), pos2(start.x, bottom)] {
            mesh.vertices.push(Vertex {
                pos,
                uv: pos2(0.5, v(pos.y)),
                color: Color32::WHITE,
            });
        }

        mesh.add_triangle(index, index + 1, index + 2);
        mesh.add_triangle(index, index + 2, index + 3);
    }

    /// Texture id of the plot gradient. The fill colors are fixed for this editor.
    fn fill_texture(&mut self, ctx: &Context) -> TextureId {
        if let Some(fill) = &self.fill {
            return fill.id();
        }

        let handle = ctx.load_texture(
            "segments-fill",
            ColorImage::new(
                [1, 2],
                vec![
                    Self::with_alpha(self.wave_color, FILL_TOP_ALPHA),
                    Self::with_alpha(self.wave_color, FILL_BOTTOM_ALPHA),
                ],
            ),
            TextureOptions::LINEAR,
        );
        let id = handle.id();

        self.fill = Some(handle);
        id
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

    fn paint_point(&self, ui: &Ui, plot: Rect, segments: &Segments, index: usize) {
        let point = segments.point(index);
        let center = Self::screen_pos(plot, Self::curve_point(point));
        let marker = self.marker(segments, index);
        let color = marker.map_or(POINT_COLOR, |(_, color)| color);
        let painter = ui.painter();

        painter.circle_filled(center, POINT_RADIUS, color);

        if let Some((label, color)) = marker {
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

        self.paint_handle(ui, segments, center, GRAB_RADIUS, Handle::Point(index));
    }

    fn paint_power_point(&self, ui: &Ui, plot: Rect, segments: &Segments, index: usize) {
        if !self.power_point_active(plot, segments, index) {
            return;
        }

        let center = Self::screen_pos(plot, self.power_position(segments, index));

        ui.painter()
            .circle_filled(center, POWER_RADIUS, self.power_color);
        self.paint_handle(
            ui,
            segments,
            center,
            POWER_GRAB_RADIUS,
            Handle::Power(index),
        );
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

    /// Hover ring, and a filled disc while dragging. Drawn on the handle layer.
    /// A dragged breakpoint also shows its level.
    fn paint_handle(
        &self,
        ui: &Ui,
        segments: &Segments,
        center: Pos2,
        radius: f32,
        handle: Handle,
    ) {
        if self.painting(ui) {
            return;
        }

        let dragging = self.dragged() == Some(handle);
        let pointer = ui.input(|input| input.pointer.hover_pos());
        let hovered =
            self.drag.is_none() && pointer.is_some_and(|pos| pos.distance(center) <= radius);

        if !dragging && !hovered {
            return;
        }

        let painter = ui.painter().clone().with_layer_id(Self::handle_layer(ui));

        if dragging {
            ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
            painter.circle_filled(center, radius, Color32::from_white_alpha(0x40));

            if let Handle::Point(index) = handle {
                let level = Units::Normalized.format(segments.point(index).y.clamp(0.0, 1.0));

                Self::paint_level(ui, center, level);
            }
        }

        if hovered {
            ui.ctx().set_cursor_icon(CursorIcon::Grab);
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
}

#[cfg(test)]
mod tests;
