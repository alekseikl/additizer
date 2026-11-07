use egui::{
    Color32, CursorIcon, DragValue, FontId, Id, Mesh, Painter, PointerButton, Pos2, Rect, Response,
    Sense, Shape, Stroke, StrokeKind, Ui, Vec2, ecolor::Hsva, epaint::Vertex, pos2, vec2,
};

use addi_dsp::{
    power_scale,
    segments::rel_segments::MAX_POWER,
    segments::time_segments::{TimePoint, TimeSegments},
    utils::sanitize,
};
use addi_engine::Smp;

use crate::units::Units;
use crate::utils::FillTexture;
use crate::value_entry::{self, ValueEntry, ValueEntryEvent};

use consts as C;
use menu::{Axis, PointAction, canvas_menu, open_menu, point_menu, power_menu};
use utils::{Ctx, Drag, Handle, bold, drag_span, stack_y, with_alpha, zoom_span};

mod consts;
mod menu;
mod utils;

/// Breakpoint coordinate being typed. The field itself only holds the text.
#[derive(Clone, Default)]
struct PointEdit {
    index: usize,
    axis: Axis,
    value: ValueEntry,
}

pub struct TimeSegmentsEditor {
    grid_y: i32,
    drag: Option<Drag>,
    drag_power: Smp,
    time_span: Smp,
    curve: Vec<[Smp; 2]>,
    scratch: Vec<Smp>,
    points: Vec<Pos2>,
    curve_cache: Option<(u64, Vec2, u32)>,
    wave_color: Color32,
    power_color: Color32,
    fill: FillTexture,
}

impl Default for TimeSegmentsEditor {
    fn default() -> Self {
        Self {
            grid_y: 4,
            drag: None,
            drag_power: 0.0,
            time_span: C::DEFAULT_SPAN,
            curve: Vec::new(),
            scratch: Vec::new(),
            points: Vec::new(),
            curve_cache: None,
            wave_color: Color32::from(C::ENVELOPE_WAVE),
            power_color: Color32::from(C::ENVELOPE_POWER),
            fill: FillTexture::new("time-segments-fill", Color32::from(C::ENVELOPE_WAVE)),
        }
    }
}

impl TimeSegmentsEditor {
    /// `playhead` is the left and right point, each `(seconds, level)` on the drawn curve.
    pub fn ui(
        &mut self,
        ui: &mut Ui,
        segments: &mut TimeSegments,
        playhead: Option<[(Smp, Smp); 2]>,
    ) -> bool {
        let revision = segments.revision();
        let size = vec2(ui.available_width(), C::EDITOR_HEIGHT);
        let (plot, response) = ui.allocate_exact_size(size, Sense::click_and_drag());

        if plot.is_positive() {
            self.plot(ui, plot, &response, segments, playhead);
        }

        ui.add_space(8.0);
        self.toolbar(ui);

        segments.revision() != revision
    }

    fn plot(
        &mut self,
        ui: &Ui,
        plot: Rect,
        response: &Response,
        segments: &mut TimeSegments,
        playhead: Option<[(Smp, Smp); 2]>,
    ) {
        let primary = ui.input(|input| input.pointer.button_down(PointerButton::Primary));

        if !primary {
            self.drag = None;
        }

        self.apply_zoom(ui, plot);

        let mut ctx = Ctx {
            ui,
            plot,
            res: response,
            segs: segments,
            span: self.time_span,
        };
        let mut remove = None;

        canvas_menu(&mut ctx);

        let points = ctx.segs.num_points();
        let active = self
            .dragged()
            .or_else(|| open_menu(&ctx))
            .or_else(|| self.hovered_handle(&ctx));

        for index in 0..points {
            if active == Some(Handle::Point(index)) && self.interact_point(&mut ctx, index) {
                remove = Some(index);
            }

            if active == Some(Handle::Power(index)) {
                self.interact_power_point(&mut ctx, index);
            }
        }

        if remove.is_none() && self.drag.is_none() && active.is_none() {
            self.add_at_double_click(&mut ctx);
        }

        if let Some(index) = remove {
            ctx.segs.remove_point(index);
            self.drag = None;
            Self::close_entry(&ctx);
        }

        ctx.span = self.time_span;

        self.show_point_entry(&mut ctx);
        self.paint_scene(&ctx, playhead);
    }

    fn add_at_double_click(&mut self, ctx: &mut Ctx<'_>) {
        if Self::editing_point(ctx).is_some() || !ctx.res.double_clicked_by(PointerButton::Primary)
        {
            return;
        }

        let Some(pos) = ctx.res.interact_pointer_pos().or(ctx.res.hover_pos()) else {
            return;
        };

        ctx.add_point(ctx.curve_pos(pos));
    }

    fn interact_power_point(&mut self, ctx: &mut Ctx<'_>, index: usize) {
        let Some(curve) = self.power_position(ctx, index) else {
            return;
        };

        let center = ctx.screen_pos(curve.x, curve.y);
        let response = ctx.handle_response(center, C::POWER_GRAB_RADIUS, (C::POWER_ID, index));

        power_menu(ctx, &response, index);

        if response.double_clicked_by(PointerButton::Primary) {
            ctx.segs.set_power(index, 0.0);
            self.drag = None;
        } else if let Some(pos) = self.drag_begin(ctx, &response) {
            self.drag = Some(Drag::new(
                ctx.curve_pos(pos),
                curve,
                Handle::Power(index),
                ctx.span,
            ));
            self.drag_power = ctx.segs.power(index);
        }

        if let Some((_, delta)) = self.drag_move(ctx, Handle::Power(index)) {
            self.move_power(ctx, index, delta.y);
        }
    }

    fn interact_point(&mut self, ctx: &mut Ctx<'_>, index: usize) -> bool {
        let point = ctx.segs.point(index);
        let time = ctx.segs.time(index);
        let center = ctx.screen_pos(time, point.y);
        let response = ctx.handle_response(center, C::GRAB_RADIUS, (C::POINT_ID, index));
        let action = point_menu(ctx, &response, index);

        if let Some(axis) = action.and_then(PointAction::axis) {
            self.open_entry(ctx, index, axis);
        }

        let mut remove = action == Some(PointAction::Remove);

        if Self::editing_point(ctx) == Some(index) {
            if remove {
                self.drag = None;
                Self::close_entry(ctx);
            }

            return remove;
        }

        if response.double_clicked_by(PointerButton::Primary) {
            remove |= ctx.is_inner(index);
        } else if let Some(pos) = self.drag_begin(ctx, &response) {
            self.drag = Some(Drag::new(
                ctx.curve_pos(pos),
                pos2(time, point.y),
                Handle::Point(index),
                self.time_span,
            ));
        }

        if let Some((placed, _)) = self.drag_move(ctx, Handle::Point(index)) {
            self.move_point(ctx, index, placed.x, placed.y);
            self.scale_past_edge(ctx, index);
        }

        if remove {
            self.drag = None;
            Self::close_entry(ctx);
        }

        remove
    }

    /// Scroll changes how many seconds the plot shows. Time 0 stays on the left.
    fn apply_zoom(&mut self, ui: &Ui, plot: Rect) {
        if self.drag.is_some() {
            return;
        }

        let over_plot = ui.input(|input| {
            input
                .pointer
                .hover_pos()
                .is_some_and(|pos| plot.contains(pos))
        });

        if !over_plot {
            return;
        }

        let scroll_y = ui.input(|input| input.smooth_scroll_delta().y);

        if scroll_y == 0.0 {
            return;
        }

        self.time_span = zoom_span(self.time_span, scroll_y);
        ui.input_mut(|input| input.smooth_scroll_delta.y = 0.0);
    }

    fn drag_begin(&self, ctx: &Ctx<'_>, response: &Response) -> Option<Pos2> {
        if self.drag.is_some() || !response.is_pointer_button_down_on() {
            return None;
        }

        let (origin, elapsed) = ctx.ui.input(|input| {
            let elapsed = input
                .pointer
                .press_start_time()
                .map_or(0.0, |start| input.time - start);

            (input.pointer.press_origin(), elapsed)
        });
        let origin = origin?;
        let pointer = response.interact_pointer_pos().or(response.hover_pos());
        let moved = pointer.is_some_and(|pos| pos.distance(origin) > C::MAX_CLICK_DIST);

        if (moved || elapsed > C::MAX_CLICK_DURATION)
            && let Some(pos) = pointer
        {
            return Some(pos);
        }

        let remaining = (C::MAX_CLICK_DURATION - elapsed).max(0.0) as f32;

        ctx.ui.ctx().request_repaint_after_secs(remaining);
        None
    }

    fn drag_move(&mut self, ctx: &Ctx<'_>, handle: Handle) -> Option<(Pos2, Vec2)> {
        if self.dragged() != Some(handle) {
            return None;
        }

        let pos = ctx.ui.input(|input| input.pointer.latest_pos())?;
        let drag = self.drag.as_ref()?;
        let span = drag.span();
        let pointer = match handle {
            Handle::Point(_) => ctx.curve_pos_at(pos, span),
            Handle::Power(_) => ctx.curve_pos(pos),
        };

        Some(drag.move_pos(pointer))
    }

    /// Keep the window still while the point is inside it. Once the point crosses the
    /// right edge, that edge follows the point.
    fn scale_past_edge(&mut self, ctx: &mut Ctx<'_>, index: usize) {
        let Some(drag) = self.drag else {
            return;
        };
        let span = drag_span(drag.span(), ctx.segs.time(index));

        self.time_span = span;
        ctx.span = span;
    }

    fn dragged(&self) -> Option<Handle> {
        self.drag.map(Drag::handle)
    }

    fn hovered_handle(&self, ctx: &Ctx<'_>) -> Option<Handle> {
        if self.drag.is_some() {
            return None;
        }

        let pointer = ctx.ui.input(|input| input.pointer.hover_pos())?;
        let mut nearest: Option<(Handle, f32)> = None;

        let mut consider = |handle, center: Pos2, radius: f32| {
            let distance = pointer.distance(center);

            if distance <= radius && nearest.is_none_or(|(_, best)| distance < best) {
                nearest = Some((handle, distance));
            }
        };

        for index in 0..ctx.segs.num_points() {
            let point = ctx.segs.point(index);

            consider(
                Handle::Point(index),
                ctx.screen_pos(ctx.segs.time(index), point.y),
                C::GRAB_RADIUS,
            );

            if let Some(curve) = self.power_position(ctx, index) {
                consider(
                    Handle::Power(index),
                    ctx.screen_pos(curve.x, curve.y),
                    C::POWER_GRAB_RADIUS,
                );
            }
        }

        nearest.map(|(handle, _)| handle)
    }

    fn move_point(&self, ctx: &mut Ctx<'_>, index: usize, time: Smp, level: Smp) {
        if index >= ctx.segs.num_points() {
            return;
        }

        let (min_time, max_time) = ctx.time_limits(index);
        let time = time.clamp(min_time, max_time);
        let mut level = level.clamp(0.0, 1.0);

        if !ctx.shift_down() {
            level = self.snap_level(ctx, level);
        }

        self.place_point(ctx, index, time, level);
    }

    /// Place point `index` at `time` seconds and `level`, clamped to the point's range.
    fn place_point(&self, ctx: &mut Ctx<'_>, index: usize, time: Smp, level: Smp) {
        if index >= ctx.segs.num_points() {
            return;
        }

        let (min_time, max_time) = ctx.time_limits(index);
        let time = time.clamp(min_time, max_time);
        let level = level.clamp(0.0, 1.0);
        let current = ctx.segs.point(index);

        ctx.segs.set_time(index, time);
        ctx.segs.set_point(
            index,
            TimePoint {
                y: level,
                ..current
            },
        );
    }

    fn open_entry(&mut self, ctx: &Ctx<'_>, index: usize, axis: Axis) {
        self.drag = None;

        let point = ctx.segs.point(index);
        let (units, value) = match axis {
            Axis::X => (Units::Time, ctx.segs.time(index)),
            Axis::Y => (Units::Normalized, point.y),
        };

        ctx.ui.data_mut(|data| {
            data.insert_temp(
                Self::entry_id(ctx),
                PointEdit {
                    index,
                    axis,
                    value: ValueEntry::new(units, value),
                },
            );
        });
    }

    fn close_entry(ctx: &Ctx<'_>) {
        ctx.ui.data_mut(|data| {
            data.remove_temp::<PointEdit>(Self::entry_id(ctx));
        });
    }

    fn entry(ctx: &Ctx<'_>) -> Option<PointEdit> {
        ctx.ui
            .data(|data| data.get_temp::<PointEdit>(Self::entry_id(ctx)))
    }

    fn editing_point(ctx: &Ctx<'_>) -> Option<usize> {
        Self::entry(ctx).map(|entry| entry.index)
    }

    fn entry_id(ctx: &Ctx<'_>) -> Id {
        ctx.ui.make_persistent_id("point-entry")
    }

    fn show_point_entry(&mut self, ctx: &mut Ctx<'_>) {
        let id = Self::entry_id(ctx);
        let Some(mut edit) = ctx.ui.data_mut(|data| data.remove_temp::<PointEdit>(id)) else {
            return;
        };
        let index = edit.index;
        let axis = edit.axis;

        if index >= ctx.segs.num_points() {
            return;
        }

        let point = ctx.segs.point(index);
        let center = ctx.screen_pos(ctx.segs.time(index), point.y);
        let (pivot, pos) = value_entry::anchor(
            ctx.ui.ctx().content_rect(),
            center,
            C::GRAB_RADIUS + C::LABEL_POINTER_GAP,
        );
        match edit.value.show(ctx.ui, id, pivot, pos) {
            ValueEntryEvent::Committed(value) => {
                let point = ctx.segs.point(index);
                let (time, level) = match axis {
                    Axis::X => (value, point.y),
                    Axis::Y => (ctx.segs.time(index), value),
                };

                self.place_point(ctx, index, time, level);

                if axis == Axis::X {
                    let span = drag_span(self.time_span, ctx.segs.time(index));

                    self.time_span = span;
                    ctx.span = span;
                }

                self.drag = None;
            }
            ValueEntryEvent::Canceled => {}
            ValueEntryEvent::Editing => {
                ctx.ui.data_mut(|data| data.insert_temp(id, edit));
            }
        }
    }

    fn snap_level(&self, ctx: &Ctx<'_>, level: Smp) -> Smp {
        let height = ctx.plot.height();

        if self.grid_y <= 1 || height <= 0.0 {
            return level;
        }

        let snapped = (level * self.grid_y as Smp).round() / self.grid_y as Smp;

        if (snapped - level).abs() * height < C::SNAP {
            snapped.clamp(0.0, 1.0)
        } else {
            level
        }
    }

    fn move_power(&self, ctx: &mut Ctx<'_>, index: usize, delta_y: Smp) {
        let end = index + 1;

        if end >= ctx.segs.num_points() {
            return;
        }

        let from = ctx.segs.point(index);
        let to = ctx.segs.point(end);

        if from.y == to.y {
            return;
        }

        let amount = C::POWER_MOUSE_MULTIPLIER * if from.y < to.y { -delta_y } else { delta_y };
        let power = sanitize(self.drag_power + amount, -MAX_POWER, MAX_POWER);

        if ctx.segs.power(index) != power {
            ctx.segs.set_power(index, power);
        }

        if !ctx.shift_down() || end + 1 >= ctx.segs.num_points() {
            return;
        }

        if ctx.segs.power(end) != -power {
            ctx.segs.set_power(end, -power);
        }
    }

    fn toolbar(&mut self, ui: &mut Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.label("Rows").on_hover_text(
                "Snap levels while dragging. Hold Shift to place them freely. Scroll the plot to zoom time.",
            );
            ui.add(DragValue::new(&mut self.grid_y).range(1..=24));
        });
    }

    fn power_position(&self, ctx: &Ctx<'_>, index: usize) -> Option<Pos2> {
        let end = index + 1;

        if end >= ctx.segs.num_points() {
            return None;
        }

        let from = ctx.segs.point(index);
        let to = ctx.segs.point(end);
        let from_time = ctx.segs.time(index);
        let to_time = from_time + ctx.segs.duration(index);
        let width = ctx.plot.width() * (to_time - from_time) / ctx.span.max(Smp::EPSILON);
        let bends = to.y != from.y && width >= C::MIN_POWER_WIDTH;

        bends.then(|| {
            let t = power_scale(0.5, ctx.segs.power(index));

            pos2((from_time + to_time) * 0.5, from.y + t * (to.y - from.y))
        })
    }

    fn build_curve_points(&mut self, ctx: &Ctx<'_>) {
        let revision = ctx.segs.revision();
        let size = ctx.plot.size();
        let span_bits = ctx.span.to_bits();

        if self.curve_cache != Some((revision, size, span_bits)) {
            let steps = size.x.round().max(1.0) as usize;

            ctx.segs
                .render_points(steps, ctx.span, &mut self.curve, &mut self.scratch);
            self.curve_cache = Some((revision, size, span_bits));
        }

        self.points.clear();
        self.points.extend(
            self.curve
                .iter()
                .map(|&[time, level]| ctx.screen_pos(time, level)),
        );
    }

    fn paint_scene(&mut self, ctx: &Ctx<'_>, playhead: Option<[(Smp, Smp); 2]>) {
        let painter = ctx.ui.painter().with_clip_rect(ctx.plot);

        self.build_curve_points(ctx);

        painter.rect_filled(ctx.plot, 2.0, ctx.ui.visuals().extreme_bg_color);
        painter.rect_stroke(
            ctx.plot,
            2.0,
            Stroke::new(1.0, Color32::from_white_alpha(32)),
            StrokeKind::Inside,
        );

        if let Some(region) = self.loop_region(ctx) {
            painter.rect_filled(region, 0.0, C::LOOP_FILL_COLOR);
        }

        self.paint_grid(&painter, ctx);

        if self.points.len() >= 2 {
            self.paint_fill(&painter, ctx);
            painter.line(self.points.clone(), Stroke::new(2.0, self.wave_color));
        }

        let hover = self.hovered_handle(ctx);
        let handles = ctx.ui.painter();

        for index in 0..ctx.segs.num_points() {
            self.paint_point(handles, ctx, index, hover);
            self.paint_power_point(handles, ctx, index, hover);
        }

        if let Some(playhead) = playhead {
            self.paint_playheads(ctx, playhead);
        }
    }

    fn paint_grid(&self, painter: &Painter, ctx: &Ctx<'_>) {
        let plot = ctx.plot;
        let stroke = Stroke::new(1.0, C::GRID_COLOR);

        self.paint_time_grid(painter, ctx);

        for i in 1..self.grid_y {
            let y = plot.bottom() - plot.height() * (i as f32 / self.grid_y as f32);

            painter.line_segment([pos2(plot.left(), y), pos2(plot.right(), y)], stroke);
        }
    }

    fn paint_time_grid(&self, painter: &Painter, ctx: &Ctx<'_>) {
        assert!(ctx.span > 0.0);

        let powers = ctx.span.log(C::RULER_DIVISION);
        let division = powers.floor();
        let transition = powers - division;
        let minor_step = C::RULER_DIVISION.powf(division - 1.0) / 2.0;
        let minor_fade = (1.0 - transition).powi(2);

        assert!(powers.is_finite() && minor_step > 0.0 && minor_step.is_finite());

        let major_every = C::RULER_DIVISION as i32;
        let mut next_label = ctx.plot.left();
        let steps = (ctx.span / minor_step).ceil() as i32;

        for idx in 1..steps {
            let fade = if idx % major_every == 0 {
                1.0
            } else {
                minor_fade
            };

            if fade < 0.02 {
                continue;
            }

            let time = idx as Smp * minor_step;
            let x = ctx.screen_x(time);

            if fade > 0.35 {
                let text = Units::Time.format(time);
                let color = with_alpha(Color32::WHITE, (0x8c as f32 * fade).round() as u8);
                let galley =
                    painter.layout_no_wrap(text, FontId::proportional(C::TIME_LABEL_SIZE), color);
                let label_x = x + C::TIME_LABEL_GAP;
                let right = label_x + galley.size().x;

                if label_x >= next_label && right <= ctx.plot.right() - 2.0 {
                    let pos = pos2(label_x, ctx.plot.bottom() - galley.size().y - 2.0);
                    painter.galley(pos, galley, color);
                    next_label = right + C::TIME_LABEL_GAP;
                }
            }

            let color = C::GRID_COLOR.gamma_multiply(fade);

            painter.line_segment(
                [pos2(x, ctx.plot.top()), pos2(x, ctx.plot.bottom())],
                Stroke::new(1.0, color),
            );
        }
    }

    fn paint_fill(&mut self, painter: &Painter, ctx: &Ctx<'_>) {
        let mut mesh = Mesh::with_texture(self.fill.id(ctx.ui.ctx()));
        let bottom = ctx.plot.bottom();

        for &[start, end] in self.points.array_windows() {
            if end.x <= start.x {
                continue;
            }

            let index = mesh.vertices.len() as u32;

            for pos in [start, end, pos2(end.x, bottom), pos2(start.x, bottom)] {
                let v = 0.25 + 0.5 * (pos.y - ctx.plot.top()) / ctx.plot.height();

                mesh.vertices.push(Vertex {
                    pos,
                    uv: pos2(0.5, v),
                    color: Color32::WHITE,
                });
            }

            mesh.add_triangle(index, index + 1, index + 2);
            mesh.add_triangle(index, index + 2, index + 3);
        }

        painter.add(Shape::mesh(mesh));
    }

    fn loop_region(&self, ctx: &Ctx<'_>) -> Option<Rect> {
        let start = ctx.segs.loop_point_idx()?;
        let end_time = ctx
            .segs
            .release_point_idx()
            .map(|index| ctx.segs.time(index))
            .unwrap_or_else(|| ctx.segs.total_time());
        let left = ctx.screen_x(ctx.segs.time(start));
        let right = ctx.screen_x(end_time);

        (right > left).then(|| Rect::from_x_y_ranges(left..=right, ctx.plot.y_range()))
    }

    fn in_span(ctx: &Ctx<'_>, time: Smp) -> bool {
        time <= ctx.span
    }

    fn paint_point(&self, painter: &Painter, ctx: &Ctx<'_>, index: usize, hover: Option<Handle>) {
        let time = ctx.segs.time(index);

        if !Self::in_span(ctx, time) {
            return;
        }

        let point = ctx.segs.point(index);
        let center = ctx.screen_pos(time, point.y);
        let marker = self.marker(ctx, index);
        let color = marker.map_or(C::POINT_COLOR, |(_, color)| color);

        painter.circle_filled(center, C::POINT_RADIUS, color);

        if let Some((label, color)) = marker {
            let galley = painter.layout_no_wrap(label.to_owned(), bold(10.0), color);
            let x = center.x - galley.size().x * 0.5;
            let y = stack_y(
                center.y,
                C::POINT_RADIUS + C::MARKER_GAP,
                galley.size().y,
                ctx.plot.top(),
            );

            painter.galley(pos2(x, y), galley, color);
        }

        self.paint_handle(ctx, center, C::GRAB_RADIUS, Handle::Point(index), hover);
    }

    fn paint_power_point(
        &self,
        painter: &Painter,
        ctx: &Ctx<'_>,
        index: usize,
        hover: Option<Handle>,
    ) {
        let Some(curve) = self.power_position(ctx, index) else {
            return;
        };

        if !Self::in_span(ctx, curve.x) {
            return;
        }

        let center = ctx.screen_pos(curve.x, curve.y);

        painter.circle_filled(center, C::POWER_RADIUS, self.power_color);
        self.paint_handle(
            ctx,
            center,
            C::POWER_GRAB_RADIUS,
            Handle::Power(index),
            hover,
        );
    }

    fn marker(&self, ctx: &Ctx<'_>, index: usize) -> Option<(&'static str, Color32)> {
        if ctx.segs.loop_point_idx() == Some(index) {
            Some(("L", C::LOOP_COLOR))
        } else if ctx.segs.release_point_idx() == Some(index) {
            Some(("R", C::RELEASE_COLOR))
        } else {
            None
        }
    }

    fn paint_handle(
        &self,
        ctx: &Ctx<'_>,
        center: Pos2,
        radius: f32,
        handle: Handle,
        hover: Option<Handle>,
    ) {
        let dragging = self.dragged() == Some(handle);
        let hovered = hover == Some(handle);

        if !dragging && !hovered {
            return;
        }

        let painter = ctx.ui.painter().clone().with_layer_id(ctx.handle_layer());

        if dragging {
            ctx.ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
            painter.circle_filled(center, radius, Color32::from_white_alpha(0x40));
        }

        if hovered {
            ctx.ui.ctx().set_cursor_icon(CursorIcon::Grab);
        }

        if let Handle::Point(index) = handle
            && Self::editing_point(ctx) != Some(index)
        {
            let point = ctx.segs.point(index);
            let time = Units::Time.format(ctx.segs.time(index));
            let level = Units::Normalized.format(point.y);

            Self::paint_label(ctx, center, format!("{time}  {level}"));
        }

        painter.circle_stroke(center, radius, Stroke::new(1.0, Color32::WHITE));
    }

    fn paint_label(ctx: &Ctx<'_>, anchor: Pos2, text: String) {
        let painter = ctx.ui.painter().clone().with_layer_id(ctx.handle_layer());
        let galley = painter.layout_no_wrap(text, bold(11.0), C::LABEL_TEXT_COLOR.into());
        let padding = vec2(C::LABEL_PADDING, C::LABEL_PADDING * 0.5);
        let size = galley.size() + padding * 2.0;
        let bounds = ctx.ui.clip_rect();
        let top = stack_y(
            anchor.y,
            C::GRAB_RADIUS + C::LABEL_POINTER_GAP,
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
            C::LABEL_CORNER_RADIUS,
            C::LABEL_BG_COLOR,
            Stroke::new(1.0, C::LABEL_BORDER_COLOR),
            StrokeKind::Inside,
        );
        painter.galley(rect.min + padding, galley, C::LABEL_TEXT_COLOR.into());
    }

    fn paint_playheads(&self, ctx: &Ctx<'_>, playhead: [(Smp, Smp); 2]) {
        let painter = ctx.ui.painter().with_clip_rect(ctx.plot);
        let [left, right] = playhead.map(|(time, level)| ctx.playhead_pos(time, level));

        if left.distance_sq(right) > C::PLAYHEAD_OVERLAP_SQ {
            let mut color = Hsva::from(self.wave_color);

            color.h = C::RIGHT_PLAYHEAD_HUE;
            painter.circle_filled(right, C::PLAYHEAD_RADIUS, color);
        }

        painter.circle_filled(left, C::PLAYHEAD_RADIUS, self.wave_color);
    }
}

#[cfg(test)]
mod tests;
