use egui::{
    Color32, ColorImage, CursorIcon, DragValue, Mesh, Painter, PointerButton, Pos2, Rect, Response,
    RichText, Sense, Shape, Stroke, StrokeKind, TextureHandle, TextureId, TextureOptions, Ui, Vec2,
    ecolor::Hsva, epaint::Vertex, pos2, vec2,
};

use addi_dsp::{
    power_scale,
    segments::MAX_POWER,
    time_segments::{TimePoint, TimeSegments, view_span},
    utils::sanitize,
};
use addi_engine::Smp;

use crate::units::Units;

use consts as C;
use menu::{canvas_menu, open_menu, point_menu, power_menu};
use utils::{Ctx, Drag, Handle, bold, stack_y, with_alpha};

mod consts;
mod menu;
mod utils;

pub struct TimeSegmentsEditor {
    grid_x: i32,
    grid_y: i32,
    drag: Option<Drag>,
    drag_power: Smp,
    /// Seconds across the plot. Frozen while a point is dragged.
    span: Smp,
    curve: Vec<[Smp; 2]>,
    scratch: Vec<Smp>,
    points: Vec<Pos2>,
    curve_cache: Option<(u64, Vec2, u32)>,
    wave_color: Color32,
    power_color: Color32,
    fill: Option<TextureHandle>,
}

impl Default for TimeSegmentsEditor {
    fn default() -> Self {
        Self {
            grid_x: 8,
            grid_y: 4,
            drag: None,
            drag_power: 0.0,
            span: view_span(1.0),
            curve: Vec::new(),
            scratch: Vec::new(),
            points: Vec::new(),
            curve_cache: None,
            wave_color: Color32::from(C::ENVELOPE_WAVE),
            power_color: Color32::from(C::ENVELOPE_POWER),
            fill: None,
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
        ui.add_space(4.0);
        ui.label(RichText::new(C::HELP).small().weak());

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

        if self.drag.is_none() {
            self.span = view_span(segments.total_time());
        }

        let mut ctx = Ctx {
            ui,
            plot,
            res: response,
            segs: segments,
            span: self.span,
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
        }

        if self.drag.is_none() {
            self.span = view_span(ctx.segs.total_time());
            ctx.span = self.span;
        }

        self.paint_scene(&ctx, playhead);
    }

    fn add_at_double_click(&mut self, ctx: &mut Ctx<'_>) {
        if !ctx.res.double_clicked_by(PointerButton::Primary) {
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
            self.drag = Some(Drag::new(ctx.curve_pos(pos), curve, Handle::Power(index)));
            self.drag_power = ctx.segs.power(index);
        }

        if let Some((_, delta)) = self.drag_move(ctx, Handle::Power(index)) {
            self.move_power(ctx, index, delta.y);
        }
    }

    fn interact_point(&mut self, ctx: &mut Ctx<'_>, index: usize) -> bool {
        let point = ctx.segs.point(index);
        let center = ctx.screen_pos(point.time, point.y);
        let response = ctx.handle_response(center, C::GRAB_RADIUS, (C::POINT_ID, index));
        let mut remove = point_menu(ctx, &response, index);

        if response.double_clicked_by(PointerButton::Primary) {
            remove |= ctx.is_inner(index);
        } else if let Some(pos) = self.drag_begin(ctx, &response) {
            self.drag = Some(Drag::new(
                ctx.curve_pos(pos),
                pos2(point.time, point.y),
                Handle::Point(index),
            ));
        }

        if let Some((placed, _)) = self.drag_move(ctx, Handle::Point(index)) {
            self.move_point(ctx, index, placed.x, placed.y);
        }

        if remove {
            self.drag = None;
        }

        remove
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
        let drag = self.drag.as_mut()?;

        Some(drag.move_pos(ctx.curve_pos(pos)))
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
                ctx.screen_pos(point.time, point.y),
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
        let mut time = time.clamp(min_time, max_time);
        let mut level = level.clamp(0.0, 1.0);

        if !ctx.shift_down() {
            time = self.snap_time(ctx, time, min_time, max_time);
            level = self.snap_level(ctx, level);
        }

        let current = ctx.segs.point(index);

        ctx.segs.set_point(
            index,
            TimePoint {
                time,
                y: level,
                ..current
            },
        );
    }

    fn snap_time(&self, ctx: &Ctx<'_>, time: Smp, min: Smp, max: Smp) -> Smp {
        let width = ctx.plot.width();

        if self.grid_x <= 1 || width <= 0.0 || ctx.span <= 0.0 {
            return time;
        }

        let snapped =
            (time / ctx.span * self.grid_x as Smp).round() / self.grid_x as Smp * ctx.span;

        if (snapped - time).abs() / ctx.span * width < C::SNAP {
            snapped.clamp(min, max)
        } else {
            time
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
            ui.label("Grid").on_hover_text(
                "Snap points while dragging. Hold Shift to place them freely. Columns divide the view.",
            );
            ui.add(DragValue::new(&mut self.grid_x).range(1..=32));
            ui.label("×");
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
        let width = ctx.plot.width() * (to.time - from.time) / ctx.span.max(Smp::EPSILON);
        let bends = to.y != from.y && width >= C::MIN_POWER_WIDTH;

        bends.then(|| {
            let t = power_scale(0.5, ctx.segs.power(index));

            pos2((from.time + to.time) * 0.5, from.y + t * (to.y - from.y))
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

        for index in 0..ctx.segs.num_points() {
            self.paint_point(ctx, index, hover);
            self.paint_power_point(ctx, index, hover);
        }

        if let Some(playhead) = playhead {
            self.paint_playheads(ctx, playhead);
        }
    }

    fn paint_grid(&self, painter: &Painter, ctx: &Ctx<'_>) {
        let plot = ctx.plot;
        let stroke = Stroke::new(1.0, C::GRID_COLOR);

        for i in 1..self.grid_x {
            let x = plot.left() + plot.width() * (i as f32 / self.grid_x as f32);

            painter.line_segment([pos2(x, plot.top()), pos2(x, plot.bottom())], stroke);
        }

        for i in 1..self.grid_y {
            let y = plot.bottom() - plot.height() * (i as f32 / self.grid_y as f32);

            painter.line_segment([pos2(plot.left(), y), pos2(plot.right(), y)], stroke);
        }
    }

    fn paint_fill(&mut self, painter: &Painter, ctx: &Ctx<'_>) {
        let mut mesh = Mesh::with_texture(self.fill_texture(ctx));
        let bottom = ctx.plot.bottom();

        for window in self.points.windows(2) {
            let (start, end) = (window[0], window[1]);

            if end.x > start.x {
                Self::add_fill_quad(&mut mesh, ctx.plot, bottom, start, end);
            }
        }

        if !mesh.is_empty() {
            painter.add(Shape::mesh(mesh));
        }
    }

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

    fn fill_texture(&mut self, ctx: &Ctx<'_>) -> TextureId {
        if let Some(fill) = &self.fill {
            return fill.id();
        }

        let handle = ctx.ui.ctx().load_texture(
            "time-segments-fill",
            ColorImage::new(
                [1, 2],
                vec![
                    with_alpha(self.wave_color, C::FILL_TOP_ALPHA),
                    with_alpha(self.wave_color, C::FILL_BOTTOM_ALPHA),
                ],
            ),
            TextureOptions::LINEAR,
        );
        let id = handle.id();

        self.fill = Some(handle);
        id
    }

    fn loop_region(&self, ctx: &Ctx<'_>) -> Option<Rect> {
        let start = ctx.segs.loop_point()?;
        let end_time = ctx
            .segs
            .release_point()
            .map(|point| point.time)
            .unwrap_or_else(|| ctx.segs.total_time());
        let left = ctx.screen_x(start.time);
        let right = ctx.screen_x(end_time);

        (right > left).then(|| Rect::from_x_y_ranges(left..=right, ctx.plot.y_range()))
    }

    fn paint_point(&self, ctx: &Ctx<'_>, index: usize, hover: Option<Handle>) {
        let point = ctx.segs.point(index);
        let center = ctx.screen_pos(point.time, point.y);
        let marker = self.marker(ctx, index);
        let color = marker.map_or(C::POINT_COLOR, |(_, color)| color);
        let painter = ctx.ui.painter();

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

    fn paint_power_point(&self, ctx: &Ctx<'_>, index: usize, hover: Option<Handle>) {
        let Some(curve) = self.power_position(ctx, index) else {
            return;
        };
        let center = ctx.screen_pos(curve.x, curve.y);

        ctx.ui
            .painter()
            .circle_filled(center, C::POWER_RADIUS, self.power_color);
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

        if let Handle::Point(index) = handle {
            let point = ctx.segs.point(index);
            let time = Units::Time.format(point.time);
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
        let painter = ctx.ui.painter();
        let [left, right] = playhead.map(|(time, level)| ctx.playhead_pos(time, level));

        if left.distance_sq(right) > C::PLAYHEAD_OVERLAP_SQ {
            let mut color = Hsva::from(self.wave_color);

            color.h = C::RIGHT_PLAYHEAD_HUE;
            painter.circle_filled(right, C::PLAYHEAD_RADIUS, color);
        }

        painter.circle_filled(left, C::PLAYHEAD_RADIUS, self.wave_color);
    }
}
