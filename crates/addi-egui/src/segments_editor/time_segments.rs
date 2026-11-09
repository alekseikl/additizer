use egui::{
    Color32, CursorIcon, DragValue, FontId, Painter, PointerButton, Pos2, Rect, Response, Sense,
    Stroke, Ui, Vec2, pos2, vec2,
};

use addi_dsp::{
    power_scale,
    segments::time_segments::{TimePoint, TimeSegments},
};
use addi_engine::Smp;

use crate::units::Units;
use crate::utils::FillTexture;
use crate::value_entry::ValueEntryEvent;

use super::common;
use consts as C;
use menu::{Axis, PointAction, canvas_menu, open_menu, point_menu, power_menu};
use utils::{
    Ctx, Drag, Handle, drag_span, paint_zoom_icon, pointer_over_zoom, with_alpha, zoom_drag,
    zoom_grab, zoom_layout,
};

mod consts;
mod menu;
mod utils;

/// Room around the curve so a 2 pt stroke centered on the plot edge is not clipped.
const PLOT_PAD: f32 = 1.0;

pub struct TimeSegmentsEditor {
    grid_y: i32,
    drag: Option<Drag>,
    zooming: bool,
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
            zooming: false,
            drag_power: 0.0,
            time_span: C::DEFAULT_SPAN,
            curve: Vec::new(),
            scratch: Vec::new(),
            points: Vec::new(),
            curve_cache: None,
            wave_color: Color32::from(C::ENVELOPE_WAVE),
            power_color: Color32::from(C::ENVELOPE_POWER),
            fill: FillTexture::new(Color32::from(C::ENVELOPE_WAVE)),
        }
    }
}

impl TimeSegmentsEditor {
    /// `playhead` is the left and right point, each `(seconds, level)` on the drawn curve.
    /// `toolbar_extra` adds widgets to the row that holds the Rows field.
    pub fn ui(
        &mut self,
        ui: &mut Ui,
        segments: &mut TimeSegments,
        playhead: Option<[(Smp, Smp); 2]>,
        toolbar_extra: impl FnOnce(&mut Ui),
    ) -> bool {
        let revision = segments.revision();
        let size = vec2(ui.available_width(), C::EDITOR_HEIGHT);
        let (plot, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
        let plot = plot.shrink(PLOT_PAD);

        if plot.is_positive() {
            self.plot(ui, plot, &response, segments, playhead);
        }

        ui.add_space(8.0);
        self.toolbar(ui, toolbar_extra);

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
            self.zooming = false;
        }

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

        if remove.is_none()
            && self.drag.is_none()
            && !self.zooming
            && active.is_none()
            && !pointer_over_zoom(ctx.ui, ctx.plot)
        {
            self.add_at_double_click(&mut ctx);
        }

        self.interact_zoom(&ctx);

        if let Some(index) = remove {
            ctx.segs.remove_point(index);
            self.drag = None;
            common::close_entry(ctx.ui);
        }

        ctx.span = self.time_span;

        self.show_point_entry(&mut ctx);
        self.paint_scene(&ctx, playhead);
    }

    fn add_at_double_click(&mut self, ctx: &mut Ctx<'_>) {
        if common::editing_index(ctx.ui).is_some()
            || !ctx.res.double_clicked_by(PointerButton::Primary)
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
            self.drag = Some(
                Drag::new(ctx.curve_pos(pos), curve, Handle::Power(index)).with_span(ctx.span),
            );
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

        if common::editing_index(ctx.ui) == Some(index) {
            if remove {
                self.drag = None;
                common::close_entry(ctx.ui);
            }

            return remove;
        }

        if response.double_clicked_by(PointerButton::Primary) {
            remove |= ctx.is_inner(index);
        } else if let Some(pos) = self.drag_begin(ctx, &response) {
            self.drag = Some(
                Drag::new(
                    ctx.curve_pos(pos),
                    pos2(time, point.y),
                    Handle::Point(index),
                )
                .with_span(self.time_span),
            );
        }

        if let Some((placed, _)) = self.drag_move(ctx, Handle::Point(index)) {
            self.move_point(ctx, index, placed.x, placed.y);
            self.scale_past_edge(ctx, index);
        }

        if remove {
            self.drag = None;
            common::close_entry(ctx.ui);
        }

        remove
    }

    /// Vertical drag of the magnifying glass changes how many seconds the plot shows.
    fn interact_zoom(&mut self, ctx: &Ctx<'_>) {
        let (_, bounds) = zoom_layout(ctx.plot);
        let response = ctx.ui.interact(
            bounds.intersect(ctx.plot),
            ctx.ui.make_persistent_id(C::ZOOM_ID),
            Sense::click_and_drag(),
        );

        if self.drag.is_some() || !response.dragged() {
            return;
        }

        self.zooming = true;
        self.time_span = zoom_drag(self.time_span, response.drag_delta().y);
    }

    fn drag_begin(&self, ctx: &Ctx<'_>, response: &Response) -> Option<Pos2> {
        common::drag_begin(self.drag.is_some(), ctx.ui, response)
    }

    fn drag_move(&mut self, ctx: &Ctx<'_>, handle: Handle) -> Option<(Pos2, Vec2)> {
        if self.dragged() != Some(handle) {
            return None;
        }

        let pos = ctx.ui.input(|input| input.pointer.latest_pos())?;
        let span = self.drag.as_ref()?.span();
        let pointer = match handle {
            Handle::Point(_) => ctx.curve_pos_at(pos, span),
            Handle::Power(_) => ctx.curve_pos(pos),
        };

        Some(self.drag.as_mut()?.move_pos(pointer))
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
        if self.drag.is_some() || self.zooming {
            return None;
        }

        let pointer = ctx.ui.input(|input| input.pointer.hover_pos())?;

        if zoom_grab(ctx.plot).contains(pointer) {
            return None;
        }
        let mut nearest: Option<(Handle, f32)> = None;

        let mut consider = |handle, center: Pos2, radius: f32| {
            common::consider(&mut nearest, handle, center, pointer, radius);
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

        common::open_entry(ctx.ui, index, axis, units, value);
    }

    fn show_point_entry(&mut self, ctx: &mut Ctx<'_>) {
        let Some(mut edit) = common::take_entry(ctx.ui) else {
            return;
        };
        let index = edit.index;
        let axis = edit.axis;

        if index >= ctx.segs.num_points() {
            return;
        }

        let point = ctx.segs.point(index);
        let center = ctx.screen_pos(ctx.segs.time(index), point.y);

        match common::show_entry(ctx.ui, &mut edit, center) {
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
            ValueEntryEvent::Editing => common::keep_entry(ctx.ui, edit),
        }
    }

    fn snap_level(&self, ctx: &Ctx<'_>, level: Smp) -> Smp {
        common::snap_division(level, self.grid_y, ctx.plot.height(), 0.0, 1.0)
    }

    fn move_power(&self, ctx: &mut Ctx<'_>, index: usize, delta_y: Smp) {
        let end = index + 1;

        if end >= ctx.segs.num_points() {
            return;
        }

        let from = ctx.segs.point(index);
        let to = ctx.segs.point(end);
        let Some(power) = common::bend_power(from.y, to.y, self.drag_power, delta_y) else {
            return;
        };

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

    fn toolbar(&mut self, ui: &mut Ui, extra: impl FnOnce(&mut Ui)) {
        ui.horizontal_wrapped(|ui| {
            ui.label("Rows").on_hover_text(
                "Snap levels while dragging. Hold Shift to place them freely. Drag the magnifying glass to zoom time.",
            );
            ui.add(DragValue::new(&mut self.grid_y).range(1..=24));
            ui.add_space(8.0);
            extra(ui);
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
        let frame = plot_frame(ctx.plot);
        let painter = ctx.ui.painter().with_clip_rect(frame);

        self.build_curve_points(ctx);

        common::paint_frame(&painter, frame, ctx.ui.visuals().extreme_bg_color);

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

        self.paint_zoom(ctx);
    }

    fn paint_zoom(&self, ctx: &Ctx<'_>) {
        let (center, _) = zoom_layout(ctx.plot);
        let painter = ctx.ui.painter().with_clip_rect(plot_frame(ctx.plot));
        let hot = self.zooming || pointer_over_zoom(ctx.ui, ctx.plot);

        paint_zoom_icon(&painter, center, Color32::from_gray(128));

        if hot {
            ctx.ui.ctx().set_cursor_icon(CursorIcon::ResizeVertical);
        }
    }

    fn paint_grid(&self, painter: &Painter, ctx: &Ctx<'_>) {
        self.paint_time_grid(painter, ctx);
        common::paint_rows(painter, ctx.plot, self.grid_y);
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
        common::paint_fill(
            painter,
            ctx.plot,
            self.fill.id(ctx.ui.ctx()),
            &self.points,
            ctx.plot.bottom(),
        );
    }

    fn loop_region(&self, ctx: &Ctx<'_>) -> Option<Rect> {
        let start = ctx.segs.loop_point_idx()?;
        let end_time = ctx
            .segs
            .release_point_idx()
            .map(|index| ctx.segs.time(index))
            .unwrap_or_else(|| ctx.segs.total_time());
        common::horizontal_span(
            ctx.plot,
            ctx.screen_x(ctx.segs.time(start)),
            ctx.screen_x(end_time),
        )
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
            common::paint_marker(painter, center, label, color, ctx.plot.top());
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
        let label = (dragging || hovered)
            .then(|| match handle {
                Handle::Point(index) if common::editing_index(ctx.ui) != Some(index) => {
                    let point = ctx.segs.point(index);
                    let time = Units::Time.format(ctx.segs.time(index));
                    let level = Units::Normalized.format(point.y);

                    Some(format!("{time}  {level}"))
                }
                _ => None,
            })
            .flatten();

        common::paint_handle(
            ctx.ui,
            ctx.handle_layer(),
            center,
            radius,
            dragging,
            hovered,
            label,
        );
    }

    fn paint_playheads(&self, ctx: &Ctx<'_>, playhead: [(Smp, Smp); 2]) {
        let [left, right] = playhead.map(|(time, level)| ctx.playhead_pos(time, level));

        common::paint_playheads(ctx.ui.painter(), left, right, self.wave_color);
    }
}

fn plot_frame(plot: Rect) -> Rect {
    plot.expand(PLOT_PAD)
}

#[cfg(test)]
mod tests;
