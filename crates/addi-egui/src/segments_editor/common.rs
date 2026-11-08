//! Shared breakpoint interaction: drag, menus, value entry, and plot drawing.

use addi_dsp::{
    segments::rel_segments::{MAX_POWER, RelSegments},
    segments::time_segments::TimeSegments,
    utils::sanitize,
};
use addi_engine::Smp;
use egui::{
    Color32, CursorIcon, FontFamily, FontId, Id, LayerId, Mesh, Order, Painter, Popup, Pos2, Rect,
    Response, Shape, Stroke, StrokeKind, TextureId, Ui, Vec2, ecolor::Hsva, epaint::Vertex,
    pos2,
};

use crate::units::Units;
use crate::utils::hsva;
use crate::value_entry::{self, ValueEntry, ValueEntryEvent};

pub const EDITOR_HEIGHT: f32 = 220.0;
pub const GRAB_RADIUS: f32 = 8.0;
pub const POWER_GRAB_RADIUS: f32 = 6.0;
pub const POINT_RADIUS: f32 = 4.0;
pub const PLAYHEAD_RADIUS: f32 = 6.0;
/// Right playhead draws once it is more than half a pixel from the left.
pub const PLAYHEAD_OVERLAP_SQ: f32 = 0.25;
/// Yellow. Saturation and value are taken from the left playhead.
pub const RIGHT_PLAYHEAD_HUE: f32 = 1.0 / 6.0;
pub const POWER_RADIUS: f32 = 3.0;
pub const MIN_POWER_WIDTH: f32 = 3.0;
/// Snap when the pointer is within this many pixels of a grid line.
pub const SNAP: f32 = 8.0;
/// Power change for a pointer move of one editor height. Matches Vital's line editor.
pub const POWER_MOUSE_MULTIPLIER: Smp = 9.0;
/// A press stays a click until the pointer moves this far.
pub const MAX_CLICK_DIST: f32 = 2.0;
/// A press stays a click until it has been held this long.
pub const MAX_CLICK_DURATION: f64 = 0.2;

pub const LOOP_COLOR: Color32 = Color32::from_rgb(0xf0, 0xc0, 0x4a);
pub const LOOP_FILL_COLOR: Hsva = hsva(0.0, 0.9092, 1.0, 0.05);
pub const RELEASE_COLOR: Color32 = Color32::from_rgb(0xff, 0x7a, 0x59);
pub const ENVELOPE_WAVE: Hsva = hsva(0.3374, 0.9, 0.2, 1.0);
pub const ENVELOPE_POWER: Hsva = hsva(0.348, 0.7, 0.4, 1.0);
pub const GRID_COLOR: Color32 = Color32::from_rgba_unmultiplied_const(255, 255, 255, 28);
pub const POINT_COLOR: Color32 = Color32::from_rgb(0xe8, 0xf4, 0xff);
pub const LABEL_BG_COLOR: Hsva = hsva(0.115, 0.05, 0.01, 1.0);
pub const LABEL_BORDER_COLOR: Hsva = hsva(0.115, 0.05, 0.2, 1.0);
pub const LABEL_TEXT_COLOR: Hsva = hsva(0.115, 0.05, 0.5, 1.0);
pub const LABEL_PADDING: f32 = 6.0;
pub const LABEL_POINTER_GAP: f32 = 4.0;
pub const LABEL_CORNER_RADIUS: f32 = 2.0;
/// Space between a point and its loop or release letter.
pub const MARKER_GAP: f32 = 3.0;

pub const SMOOTH_TIP: &str = "Ease this segment with a sine curve.";
pub const LOOP_TIP: &str = "Repeat from here. With no release point the loop runs to the end. With one, it runs until release while the key is held.";
pub const RELEASE_TIP: &str = "Sustain this level while the key is held. On release, continue from here starting at the current level.";

/// Handle held by the pointer.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Handle {
    Point(usize),
    Power(usize),
}

/// Pointer grab of a circle. `pointer` and `origin` are where the drag started.
/// Later centers keep that grab offset. `span` is the time window at the grab,
/// so a time editor can grow the window without the point running away.
#[derive(Clone, Copy, PartialEq)]
pub struct Drag {
    pointer: Pos2,
    origin: Pos2,
    center: Pos2,
    handle: Handle,
    span: Smp,
}

impl Drag {
    pub fn new(pointer: Pos2, center: Pos2, handle: Handle) -> Self {
        Self {
            pointer,
            origin: center,
            center,
            handle,
            span: 0.0,
        }
    }

    pub fn with_span(mut self, span: Smp) -> Self {
        self.span = span;
        self
    }

    pub fn handle(self) -> Handle {
        self.handle
    }

    pub fn span(self) -> Smp {
        self.span
    }

    /// Circle center after the pointer moves to `pointer`, keeping the grab offset from `origin`.
    pub fn place(self, pointer: Pos2) -> Pos2 {
        pos2(
            self.origin.x + pointer.x - self.pointer.x,
            self.origin.y + pointer.y - self.pointer.y,
        )
    }

    /// Moves the center to follow `pointer`.
    /// Returns the new center and its offset from `origin`.
    pub fn move_pos(&mut self, pointer: Pos2) -> (Pos2, Vec2) {
        let center = self.place(pointer);
        let delta = center - self.origin;

        self.center = center;
        (center, delta)
    }
}

/// Coordinate of a breakpoint the menu asked to edit.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum Axis {
    #[default]
    X,
    Y,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PointAction {
    Remove,
    X,
    Y,
}

impl PointAction {
    pub fn axis(self) -> Option<Axis> {
        match self {
            Self::X => Some(Axis::X),
            Self::Y => Some(Axis::Y),
            Self::Remove => None,
        }
    }
}

/// Breakpoint coordinate being typed.
#[derive(Clone, Default)]
pub struct PointEdit {
    pub index: usize,
    pub axis: Axis,
    pub value: ValueEntry,
}

/// Curve operations shared by the point and bend menus.
pub trait CurveMarks {
    fn point_count(&self) -> usize;
    fn smooth_at(&self, index: usize) -> bool;
    fn assign_smooth(&mut self, index: usize, smooth: bool);
    fn assign_power(&mut self, index: usize, power: Smp);
    fn clear_powers(&mut self);
    fn loop_at(&self) -> Option<usize>;
    fn release_at(&self) -> Option<usize>;
    fn assign_loop(&mut self, index: Option<usize>);
    fn assign_release(&mut self, index: Option<usize>);
}

impl CurveMarks for RelSegments {
    fn point_count(&self) -> usize {
        self.num_points()
    }

    fn smooth_at(&self, index: usize) -> bool {
        self.point(index).curve.smooth
    }

    fn assign_smooth(&mut self, index: usize, smooth: bool) {
        self.set_smooth(index, smooth);
    }

    fn assign_power(&mut self, index: usize, power: Smp) {
        self.set_power(index, power);
    }

    fn clear_powers(&mut self) {
        self.reset_powers();
    }

    fn loop_at(&self) -> Option<usize> {
        self.loop_point_idx()
    }

    fn release_at(&self) -> Option<usize> {
        self.release_point_idx()
    }

    fn assign_loop(&mut self, index: Option<usize>) {
        self.set_loop_point(index);
    }

    fn assign_release(&mut self, index: Option<usize>) {
        self.set_release_point(index);
    }
}

impl CurveMarks for TimeSegments {
    fn point_count(&self) -> usize {
        self.num_points()
    }

    fn smooth_at(&self, index: usize) -> bool {
        self.point(index).curve.smooth
    }

    fn assign_smooth(&mut self, index: usize, smooth: bool) {
        self.set_smooth(index, smooth);
    }

    fn assign_power(&mut self, index: usize, power: Smp) {
        self.set_power(index, power);
    }

    fn clear_powers(&mut self) {
        self.reset_powers();
    }

    fn loop_at(&self) -> Option<usize> {
        self.loop_point_idx()
    }

    fn release_at(&self) -> Option<usize> {
        self.release_point_idx()
    }

    fn assign_loop(&mut self, index: Option<usize>) {
        self.set_loop_point(index);
    }

    fn assign_release(&mut self, index: Option<usize>) {
        self.set_release_point(index);
    }
}

/// Handle whose context menu is open.
pub fn open_handle(
    ui: &Ui,
    points: usize,
    point_id: &str,
    power_id: &str,
) -> Option<Handle> {
    (0..points).find_map(|index| {
        if popup_open(ui, (point_id, index)) {
            Some(Handle::Point(index))
        } else if popup_open(ui, (power_id, index)) {
            Some(Handle::Power(index))
        } else {
            None
        }
    })
}

fn popup_open(ui: &Ui, salt: impl std::hash::Hash + std::fmt::Debug) -> bool {
    let id = ui.make_persistent_id(salt).with("popup");

    Popup::is_id_open(ui.ctx(), id)
}

/// What the menu asked to do with this point. `x_label` is "Set phase" or "Set time".
pub fn point_menu(
    response: &Response,
    segments: &mut impl CurveMarks,
    index: usize,
    x_label: &'static str,
    markers: bool,
) -> Option<PointAction> {
    let mut action = None;

    response.context_menu(|ui| {
        if index >= segments.point_count() {
            return;
        }

        if ui.button(x_label).clicked() {
            action = Some(PointAction::X);
            ui.close();
        }

        if ui.button("Set level").clicked() {
            action = Some(PointAction::Y);
            ui.close();
        }

        smooth_item(ui, segments, index);

        if markers {
            marker_items(ui, segments, index);
        }

        if is_inner(segments.point_count(), index) && ui.button("Remove point").clicked() {
            action = Some(PointAction::Remove);
            ui.close();
        }
    });

    action
}

pub fn power_menu(response: &Response, segments: &mut impl CurveMarks, index: usize) {
    response.context_menu(|ui| {
        if index < segments.point_count() {
            smooth_item(ui, segments, index);
        }

        if ui.button("Reset bend").clicked() {
            segments.assign_power(index, 0.0);
            ui.close();
        }
    });
}

pub fn reset_bends_item(ui: &mut Ui, segments: &mut impl CurveMarks) {
    if ui.button("Reset bends").clicked() {
        segments.clear_powers();
        ui.close();
    }
}

/// Toggles the sine easing of segment `index`.
fn smooth_item(ui: &mut Ui, segments: &mut impl CurveMarks, index: usize) {
    let smooth = segments.smooth_at(index);
    let label = if smooth { "Flat" } else { "Smooth" };

    if ui.button(label).on_hover_text(SMOOTH_TIP).clicked() {
        segments.assign_smooth(index, !smooth);
        ui.close();
    }
}

/// Loop and release choices for point `index`.
fn marker_items(ui: &mut Ui, segments: &mut impl CurveMarks, index: usize) {
    if segments.loop_at() == Some(index) {
        if ui.button("Clear loop").clicked() {
            segments.assign_loop(None);
            ui.close();
        }
    } else {
        let can_loop = index + 1 < segments.point_count()
            && segments.release_at().is_none_or(|release| index < release);

        if can_loop && ui.button("Loop point").on_hover_text(LOOP_TIP).clicked() {
            segments.assign_loop(Some(index));
            ui.close();
        }
    }

    if segments.release_at() == Some(index) {
        if ui.button("Clear release").clicked() {
            segments.assign_release(None);
            ui.close();
        }
    } else {
        let can_release = index > 0
            && index + 1 < segments.point_count()
            && segments
                .loop_at()
                .is_none_or(|loop_point| index > loop_point);

        if can_release
            && ui
                .button("Release point")
                .on_hover_text(RELEASE_TIP)
                .clicked()
        {
            segments.assign_release(Some(index));
            ui.close();
        }
    }
}

pub fn entry_id(ui: &Ui) -> Id {
    ui.make_persistent_id("point-entry")
}

pub fn open_entry(ui: &Ui, index: usize, axis: Axis, units: Units, value: Smp) {
    let id = entry_id(ui);

    ui.data_mut(|data| {
        data.insert_temp(
            id,
            PointEdit {
                index,
                axis,
                value: ValueEntry::new(units, value),
            },
        );
    });
}

pub fn close_entry(ui: &Ui) {
    let id = entry_id(ui);

    ui.data_mut(|data| {
        data.remove_temp::<PointEdit>(id);
    });
}

pub fn editing_index(ui: &Ui) -> Option<usize> {
    let id = entry_id(ui);

    ui.data(|data| data.get_temp::<PointEdit>(id))
        .map(|entry| entry.index)
}

pub fn take_entry(ui: &Ui) -> Option<PointEdit> {
    let id = entry_id(ui);

    ui.data_mut(|data| data.remove_temp::<PointEdit>(id))
}

pub fn keep_entry(ui: &Ui, edit: PointEdit) {
    let id = entry_id(ui);

    ui.data_mut(|data| data.insert_temp(id, edit));
}

/// Foreground field anchored above `center`.
pub fn show_entry(ui: &Ui, edit: &mut PointEdit, center: Pos2) -> ValueEntryEvent {
    let id = entry_id(ui);
    let (pivot, pos) = value_entry::anchor(
        ui.ctx().content_rect(),
        center,
        GRAB_RADIUS + LABEL_POINTER_GAP,
    );

    edit.value.show(ui, id, pivot, pos)
}

pub fn shift_down(ui: &Ui) -> bool {
    ui.input(|input| input.modifiers.shift)
}

pub fn handle_layer(ui: &Ui, salt: &str) -> LayerId {
    LayerId::new(Order::Foreground, ui.scope_id().with(salt))
}

/// Pointer once this press is a drag and no other handle is held.
pub fn drag_begin(held: bool, ui: &Ui, response: &Response) -> Option<Pos2> {
    if held || !response.is_pointer_button_down_on() {
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

/// Keeps the closer handle. A tie keeps the one already stored.
pub fn consider(
    nearest: &mut Option<(Handle, f32)>,
    handle: Handle,
    center: Pos2,
    pointer: Pos2,
    radius: f32,
) {
    let distance = pointer.distance(center);

    if distance <= radius && nearest.is_none_or(|(_, best)| distance < best) {
        *nearest = Some((handle, distance));
    }
}

/// Snap `value` to `divisions` when it is within [`SNAP`] pixels across `pixels`.
pub fn snap_division(value: Smp, divisions: i32, pixels: f32, min: Smp, max: Smp) -> Smp {
    if divisions <= 1 || pixels <= 0.0 {
        return value;
    }

    let snapped = (value * divisions as Smp).round() / divisions as Smp;

    if (snapped - value).abs() * pixels < SNAP {
        snapped.clamp(min, max)
    } else {
        value
    }
}

/// New bend after a vertical drag of `delta_y`. `None` when the segment is flat.
pub fn bend_power(from_y: Smp, to_y: Smp, drag_power: Smp, delta_y: Smp) -> Option<Smp> {
    if from_y == to_y {
        return None;
    }

    let amount = POWER_MOUSE_MULTIPLIER * if from_y < to_y { -delta_y } else { delta_y };

    Some(sanitize(drag_power + amount, -MAX_POWER, MAX_POWER))
}

/// Interior points can be removed. The ends stay.
pub fn is_inner(count: usize, index: usize) -> bool {
    index > 0 && index + 1 < count
}

pub fn paint_frame(painter: &Painter, plot: Rect, background: Color32) {
    painter.rect_filled(plot, 2.0, background);
    painter.rect_stroke(
        plot,
        2.0,
        Stroke::new(1.0, Color32::from_white_alpha(32)),
        StrokeKind::Inside,
    );
}

pub fn paint_rows(painter: &Painter, plot: Rect, rows: i32) {
    let stroke = Stroke::new(1.0, GRID_COLOR);

    for i in 1..rows {
        let y = plot.bottom() - plot.height() * (i as f32 / rows as f32);

        painter.line_segment([pos2(plot.left(), y), pos2(plot.right(), y)], stroke);
    }
}

/// Area under the polyline, shaded by a 1×2 gradient of the full plot.
pub fn paint_fill(painter: &Painter, plot: Rect, texture: TextureId, points: &[Pos2]) {
    if plot.height() <= 0.0 {
        return;
    }

    let mut mesh = Mesh::with_texture(texture);
    let bottom = plot.bottom();

    for &[start, end] in points.array_windows() {
        if end.x > start.x {
            add_fill_quad(&mut mesh, plot, bottom, start, end);
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

pub fn paint_marker(
    painter: &Painter,
    center: Pos2,
    label: &str,
    color: Color32,
    plot_top: f32,
) {
    let galley = painter.layout_no_wrap(label.to_owned(), bold(10.0), color);
    let x = center.x - galley.size().x * 0.5;
    let y = stack_y(
        center.y,
        POINT_RADIUS + MARKER_GAP,
        galley.size().y,
        plot_top,
    );

    painter.galley(pos2(x, y), galley, color);
}

/// Hover ring, and a filled disc while dragging. `label` is the readout for a breakpoint.
pub fn paint_handle(
    ui: &Ui,
    layer: LayerId,
    center: Pos2,
    radius: f32,
    dragging: bool,
    hovered: bool,
    label: Option<String>,
) {
    if !dragging && !hovered {
        return;
    }

    if let Some(label) = label {
        paint_label(ui, layer, center, label);
    }

    let painter = ui.painter().clone().with_layer_id(layer);

    if dragging {
        ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
        painter.circle_filled(center, radius, Color32::from_white_alpha(0x40));
    }

    if hovered {
        ui.ctx().set_cursor_icon(CursorIcon::Grab);
    }

    painter.circle_stroke(center, radius, Stroke::new(1.0, Color32::WHITE));
}

/// Readout next to `anchor`, kept inside the clip rect.
pub fn paint_label(ui: &Ui, layer: LayerId, anchor: Pos2, text: String) {
    let painter = ui.painter().clone().with_layer_id(layer);
    let galley = painter.layout_no_wrap(text, bold(11.0), LABEL_TEXT_COLOR.into());
    let padding = vec2_padding();
    let size = galley.size() + padding * 2.0;
    let bounds = ui.clip_rect();
    let top = stack_y(
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

fn vec2_padding() -> Vec2 {
    egui::vec2(LABEL_PADDING, LABEL_PADDING * 0.5)
}

/// Left stays `wave`. The right channel keeps that saturation and value.
pub fn paint_playheads(painter: &Painter, left: Pos2, right: Pos2, wave: Color32) {
    if left.distance_sq(right) > PLAYHEAD_OVERLAP_SQ {
        let mut color = Hsva::from(wave);

        color.h = RIGHT_PLAYHEAD_HUE;
        painter.circle_filled(right, PLAYHEAD_RADIUS, color);
    }

    painter.circle_filled(left, PLAYHEAD_RADIUS, wave);
}

/// Screen span from `left` to `right` when the loop has width.
pub fn horizontal_span(plot: Rect, left: f32, right: f32) -> Option<Rect> {
    (right > left).then(|| Rect::from_x_y_ranges(left..=right, plot.y_range()))
}

/// Top of a box `height` tall placed `offset` above `anchor_y`, or `offset` below it
/// when the box would pass `top`.
pub fn stack_y(anchor_y: f32, offset: f32, height: f32, top: f32) -> f32 {
    let above = anchor_y - offset - height;

    if above >= top {
        above
    } else {
        anchor_y + offset
    }
}

pub fn bold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("Bold".into()))
}

pub fn with_alpha(color: Color32, alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha)
}

#[cfg(test)]
mod tests;
