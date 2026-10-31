use egui::{
    Color32, ColorImage, CursorIcon, DragValue, Mesh, Painter, PointerButton, Pos2, Rect, Response,
    RichText, Sense, Shape, Stroke, StrokeKind, TextureHandle, TextureId, TextureOptions, Ui, Vec2,
    ecolor::Hsva, epaint::Vertex, pos2, vec2,
};

use addi_dsp::{
    power_scale,
    segments::{MAX_POINTS, MAX_POWER, SegPoint, Segments},
    utils::sanitize,
};
use addi_engine::{Smp, StereoSample};

use crate::units::Units;

use consts as C;
use menu::{canvas_menu, open_menu, point_menu, power_menu};
use utils::{
    Ctx, Drag, Handle, TOOLS, Tool, bold, curve_point, paint_edit_icon, paint_pattern_icon,
    stack_y, with_alpha,
};

mod consts;
mod menu;
mod utils;

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
    /// Grid column under the pointer while stamping.
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
            paint_column: None,
            wave_color: C::WAVE_COLOR,
            power_color: C::POWER_COLOR,
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
            wave_color: Color32::from(C::ENVELOPE_WAVE),
            power_color: Color32::from(C::ENVELOPE_POWER),
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
        let size = vec2(ui.available_width(), C::EDITOR_HEIGHT);
        let (plot, response) = ui.allocate_exact_size(size, Sense::click_and_drag());

        if plot.is_positive() {
            self.plot(
                &mut Ctx {
                    ui,
                    plot,
                    res: &response,
                    stamping: matches!(self.tool, Tool::Paint(_)),
                    segs: segments,
                },
                playhead,
            );
        }

        ui.add_space(8.0);
        self.toolbar(ui);
        ui.add_space(4.0);
        ui.label(RichText::new(self.help()).small().weak());

        segments.revision() != revision
    }

    fn help(&self) -> &'static str {
        if matches!(self.tool, Tool::Paint(_)) {
            C::HELP_PAINT
        } else if self.markers {
            C::HELP_MARKERS
        } else {
            C::HELP
        }
    }

    fn plot(&mut self, ctx: &mut Ctx<'_>, playhead: Option<[(Smp, Smp); 2]>) {
        let mut remove = None;

        if ctx.stamping || !ctx.ui.input(|input| input.pointer.primary_down()) {
            self.drag = None;
        }

        if ctx.stamping {
            self.paint_pointer(ctx);
            canvas_menu(ctx);
        } else {
            self.paint_column = None;
            Self::interact_canvas(ctx);

            let points = ctx.segs.num_points();
            let active = self
                .dragged()
                .or_else(|| open_menu(ctx))
                .or_else(|| self.hovered_handle(ctx));

            for index in 0..points {
                if active == Some(Handle::Point(index)) && self.interact_point(ctx, index) {
                    remove = Some(index);
                }

                if active == Some(Handle::Power(index)) {
                    self.interact_power_point(ctx, index);
                }
            }
        }

        self.paint_scene(ctx);

        if ctx.stamping && (ctx.res.hovered() || ctx.res.is_pointer_button_down_on()) {
            ctx.ui.ctx().set_cursor_icon(CursorIcon::Crosshair);
        }

        if let Some(playhead) = playhead {
            self.paint_playheads(ctx, playhead);
        }

        if let Some(index) = remove {
            ctx.segs.remove_point(index);
        }
    }

    fn interact_canvas(ctx: &mut Ctx<'_>) {
        canvas_menu(ctx);

        if ctx.res.double_clicked_by(PointerButton::Primary)
            && let Some(pos) = ctx.res.interact_pointer_pos().or(ctx.res.hover_pos())
        {
            ctx.add_point(ctx.curve_pos(pos));
        }
    }

    /// One power point: drag, double-click, and menu.
    fn interact_power_point(&mut self, ctx: &mut Ctx<'_>, index: usize) {
        let Some(curve) = self.power_position(ctx, index) else {
            return;
        };

        let center = ctx.screen_pos(curve);
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

    /// One breakpoint: drag, double-click, and menu.
    fn interact_point(&mut self, ctx: &mut Ctx<'_>, index: usize) -> bool {
        let center = ctx.screen_pos(curve_point(ctx.segs.point(index)));
        let response = ctx.handle_response(center, C::GRAB_RADIUS, (C::POINT_ID, index));
        let mut remove = point_menu(self.markers, ctx, &response, index);

        if response.double_clicked_by(PointerButton::Primary) {
            remove |= ctx.is_inner(index);
        } else if let Some(pos) = self.drag_begin(ctx, &response) {
            self.drag = Some(Drag::new(
                ctx.curve_pos(pos),
                curve_point(ctx.segs.point(index)),
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

    /// Pointer once this press is a drag and no other handle is held.
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

    /// Advances `handle` while the primary button is held.
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

    /// Nearest handle whose grab contains the pointer. A tie keeps the earlier one.
    /// `None` while dragging, so the held handle stays the only active one.
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
            consider(
                Handle::Point(index),
                ctx.screen_pos(curve_point(ctx.segs.point(index))),
                C::GRAB_RADIUS,
            );

            if let Some(curve) = self.power_position(ctx, index) {
                consider(
                    Handle::Power(index),
                    ctx.screen_pos(curve),
                    C::POWER_GRAB_RADIUS,
                );
            }
        }

        nearest.map(|(handle, _)| handle)
    }

    /// Shift places the point freely.
    fn move_point(&self, ctx: &mut Ctx<'_>, index: usize, x: Smp, y: Smp) {
        if index >= ctx.segs.num_points() {
            return;
        }

        let (min_x, max_x) = ctx.x_limits(index);
        let mut x = x.clamp(min_x, max_x);
        let mut y = y.clamp(0.0, 1.0);

        if !ctx.shift_down() {
            x = self.snap_x(ctx, x, min_x, max_x);
            y = self.snap_y(ctx, y);
        }

        self.match_wrapped_ends(ctx, index, y);

        let current = ctx.segs.point(index);
        ctx.segs.set_point(index, SegPoint { x, y, ..current });
    }

    /// Snap `x` to the vertical grid when it is within [`C::SNAP`] pixels.
    fn snap_x(&self, ctx: &Ctx<'_>, x: Smp, min: Smp, max: Smp) -> Smp {
        let width = ctx.plot.width();

        if self.grid_x <= 1 || width <= 0.0 {
            return x;
        }

        let snapped = (x * self.grid_x as Smp).round() / self.grid_x as Smp;

        if (snapped - x).abs() * width < C::SNAP {
            snapped.clamp(min, max)
        } else {
            x
        }
    }

    /// Snap `y` to the horizontal grid when it is within [`C::SNAP`] pixels.
    fn snap_y(&self, ctx: &Ctx<'_>, y: Smp) -> Smp {
        let height = ctx.plot.height();

        if self.grid_y <= 1 || height <= 0.0 {
            return y;
        }

        let snapped = (y * self.grid_y as Smp).round() / self.grid_y as Smp;

        if (snapped - y).abs() * height < C::SNAP {
            snapped.clamp(0.0, 1.0)
        } else {
            y
        }
    }

    /// Wrapped curves keep the first and last levels equal.
    fn match_wrapped_ends(&self, ctx: &mut Ctx<'_>, index: usize, y: Smp) {
        if !self.wrapped {
            return;
        }

        let n = ctx.segs.num_points();
        let other = if index == 0 {
            n - 1
        } else if index + 1 == n {
            0
        } else {
            return;
        };
        let point = ctx.segs.point(other);

        ctx.segs.set_point(other, SegPoint { y, ..point });
    }

    /// Bend segment `index` from [`Self::drag_power`] by the vertical offset `delta_y`.
    /// Shift mirrors the bend onto the next segment.
    fn move_power(&self, ctx: &mut Ctx<'_>, index: usize, delta_y: Smp) {
        let Some(end) = self.segment_end(ctx, index) else {
            return;
        };
        let from = ctx.segs.point(index);

        if from.y == end.y {
            return;
        }

        let amount = C::POWER_MOUSE_MULTIPLIER * if from.y < end.y { -delta_y } else { delta_y };
        let power = sanitize(self.drag_power + amount, -MAX_POWER, MAX_POWER);

        if ctx.segs.power(index) != power {
            ctx.segs.set_power(index, power);
        }

        if !ctx.shift_down() {
            return;
        }

        let Some(next) = self.segment_end_idx(ctx, index) else {
            return;
        };

        if self.segment_end_idx(ctx, next).is_some() && ctx.segs.power(next) != -power {
            ctx.segs.set_power(next, -power);
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
        self.tool = if self.tool == tool { Tool::Edit } else { tool };
    }

    /// Square button matching the height of the other widgets, like the grid `DragValue`s.
    fn tool_button(&self, ui: &mut Ui, tool: Tool) -> Response {
        let size = Vec2::splat(ui.spacing().interact_size.y);
        let (rect, response) = ui.allocate_exact_size(size, Sense::click());
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
            rect.shrink(C::TOOL_ICON_INSET),
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
            Tool::Edit => paint_edit_icon(painter, icon, color),
            Tool::Paint(pattern) => paint_pattern_icon(painter, icon, pattern, color),
        }
    }

    /// Highlights the column under the pointer and stamps into it while the button is held.
    /// The press shows the level being stamped.
    fn paint_pointer(&mut self, ctx: &mut Ctx<'_>) {
        let pos = ctx.res.interact_pointer_pos().or(ctx.res.hover_pos());

        self.paint_column = pos.map(|pos| self.column_at(ctx, pos));

        let primary = ctx
            .ui
            .input(|input| input.pointer.button_down(PointerButton::Primary));

        if primary
            && ctx.res.is_pointer_button_down_on()
            && let Some(pos) = pos
        {
            Self::paint_level(
                ctx,
                pos,
                Units::Normalized.format(self.pointer_level(ctx, pos)),
            );
            self.stamp_column(ctx, pos);
        }
    }

    /// Shift places the level freely. Otherwise it snaps to the vertical grid.
    fn pointer_level(&self, ctx: &Ctx<'_>, pos: Pos2) -> Smp {
        let mut level = ctx.curve_pos(pos).y.clamp(0.0, 1.0);

        if !ctx.shift_down() {
            level = self.snap_y(ctx, level);
        }

        level
    }

    fn column_at(&self, ctx: &Ctx<'_>, pos: Pos2) -> i32 {
        let columns = self.grid_x.max(1);
        let x = ctx.curve_pos(pos).x.clamp(0.0, 1.0);
        let column = (x * columns as Smp).floor() as i32;

        column.clamp(0, columns - 1)
    }

    fn column_bounds(&self, column: i32) -> (Smp, Smp) {
        let columns = self.grid_x.max(1);
        let column = column.clamp(0, columns - 1) as Smp;

        (column / columns as Smp, (column + 1.0) / columns as Smp)
    }

    /// Stamps the active pattern into the grid column under `pos`.
    /// An unwrapped curve takes the pattern's edge when the stamp covers that endpoint.
    fn stamp_column(&self, ctx: &mut Ctx<'_>, pos: Pos2) {
        let Tool::Paint(pattern) = self.tool else {
            return;
        };
        let (from_x, to_x) = self.column_bounds(self.column_at(ctx, pos));
        let level = self.pointer_level(ctx, pos);
        let segments = &mut *ctx.segs;

        if !(from_x >= 0.0 && from_x < to_x && to_x <= 1.0) {
            return;
        }

        let level = level.clamp(0.0, 1.0);
        let pattern_points = pattern.scaled(from_x, to_x, level);
        let Some((&first_edge, &last_edge)) = pattern_points.first().zip(pattern_points.last())
        else {
            return;
        };
        let rewrite_first = !self.wrapped && from_x == 0.0;
        let rewrite_last = !self.wrapped && to_x == 1.0;

        let points = segments.points();
        let head_len = points.partition_point(|&point| point.x < from_x);
        let tail_at = points.partition_point(|&point| point.x <= to_x);
        let left = points[head_len].x == from_x;
        let right = points[tail_at - 1].x == to_x;
        let head_end = head_len + usize::from(left);
        let tail_start = tail_at - usize::from(right);

        let adopt = |point: SegPoint, edge: SegPoint| SegPoint {
            y: edge.y,
            power: edge.power,
            smooth: edge.smooth,
            ..point
        };
        let mut stamped = Vec::with_capacity(points.len() + pattern_points.len() + 2);

        stamped.extend_from_slice(&points[..head_end]);

        if rewrite_first {
            stamped[0] = adopt(stamped[0], first_edge);
        }

        let middle_at = stamped.len();

        if !left {
            stamped.push(SegPoint {
                x: from_x,
                y: segments.value_at(from_x, None, None),
                ..SegPoint::default()
            });
        }

        // The pattern's own edges step aside for the endpoints that adopt them.
        let pattern_from = usize::from(rewrite_first);
        let pattern_to = pattern_points.len() - usize::from(rewrite_last);

        stamped.extend_from_slice(&pattern_points[pattern_from..pattern_to]);

        if !right {
            stamped.push(SegPoint {
                x: to_x,
                y: segments.value_at(to_x, None, None),
                ..SegPoint::default()
            });
        }

        let middle_end = stamped.len();

        stamped.extend_from_slice(&points[tail_start..]);

        if rewrite_last {
            let last = stamped.len() - 1;

            stamped[last] = adopt(stamped[last], last_edge);
        }

        if stamped.len() > MAX_POINTS || segments.points() == stamped.as_slice() {
            return;
        }

        // Splice it in: the replaced run makes room for the stamped middle. Endpoints
        // cannot be removed, so a rewritten edge is patched in place instead.
        for index in (head_end..tail_start).rev() {
            assert!(segments.remove_point(index));
        }

        for (offset, point) in stamped[middle_at..middle_end].iter().enumerate() {
            assert!(segments.add_point(middle_at + offset, *point));
        }

        if rewrite_first {
            segments.set_point(0, stamped[0]);
        }

        if rewrite_last {
            let last = segments.num_points() - 1;

            segments.set_point(last, stamped[last]);
        }

        debug_assert_eq!(segments.points(), stamped.as_slice());
    }

    /// Curve-space midpoint of segment `index`, power-scaled in level. A wrap past 1 folds
    /// back into the plot. `None` when the segment is flat or too narrow to bend.
    fn power_position(&self, ctx: &Ctx<'_>, index: usize) -> Option<Pos2> {
        let from = curve_point(ctx.segs.point(index));
        let end = self.segment_end(ctx, index)?;
        let bends = end.y != from.y && ctx.plot.width() * (end.x - from.x) >= C::MIN_POWER_WIDTH;

        bends.then(|| {
            let x = (from.x + end.x) * 0.5;
            let x = if x >= 1.0 { x - 1.0 } else { x };
            let t = power_scale(0.5, ctx.segs.power(index));

            pos2(x, from.y + t * (end.y - from.y))
        })
    }

    /// Breakpoint that ends segment `index`. The last segment of a wrapped curve wraps
    /// to the start, so this is also the next segment to bend.
    fn segment_end_idx(&self, ctx: &Ctx<'_>, index: usize) -> Option<usize> {
        let n = ctx.segs.num_points();

        if index + 1 < n {
            Some(index + 1)
        } else if self.wrapped && index < n {
            Some(0)
        } else {
            None
        }
    }

    /// End of segment `index` in curve space. The closing segment ends one cycle past the start.
    fn segment_end(&self, ctx: &Ctx<'_>, index: usize) -> Option<Pos2> {
        let end = self.segment_end_idx(ctx, index)?;
        let point = ctx.segs.point(end);
        let wrap = if end == 0 { 1.0 } else { 0.0 };

        Some(pos2(point.x + wrap, point.y))
    }

    /// Polyline with one sample per horizontal pixel and every breakpoint kept.
    fn build_curve_points(&mut self, ctx: &Ctx<'_>) {
        assert!(ctx.plot.width() > 0.0);

        let revision = ctx.segs.revision();
        let size = ctx.plot.size();

        if self.curve_cache != Some((revision, size)) {
            let steps = size.x.round().max(1.0) as usize;

            ctx.segs
                .render_points(steps, &mut self.curve, &mut self.scratch);
            self.curve_cache = Some((revision, size));
        }

        self.points.clear();
        self.points
            .extend(self.curve.iter().map(|&[x, y]| ctx.screen_pos(pos2(x, y))));

        assert!(self.points.len() >= 2);
    }

    fn paint_scene(&mut self, ctx: &Ctx<'_>) {
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
        self.paint_fill(&painter, ctx);
        self.paint_column_highlight(&painter, ctx);
        painter.line(self.points.clone(), Stroke::new(2.0, self.wave_color));

        let hover = self.hovered_handle(ctx);

        for index in 0..ctx.segs.num_points() {
            self.paint_point(ctx, index, hover);
            self.paint_power_point(ctx, index, hover);
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

    fn paint_column_highlight(&self, painter: &Painter, ctx: &Ctx<'_>) {
        let Some(column) = self.paint_column else {
            return;
        };
        let (from_x, to_x) = self.column_bounds(column);
        let left = ctx.screen_x(from_x);
        let right = ctx.screen_x(to_x);

        painter.rect_filled(
            Rect::from_x_y_ranges(left..=right, ctx.plot.y_range()),
            0.0,
            with_alpha(self.wave_color, C::PAINT_COLUMN_ALPHA),
        );
    }

    /// Area under the polyline, shaded by a 1×2 gradient of the full plot.
    fn paint_fill(&mut self, painter: &Painter, ctx: &Ctx<'_>) {
        assert!(self.points.len() >= 2 && ctx.plot.height() > 0.0);

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
    fn fill_texture(&mut self, ctx: &Ctx<'_>) -> TextureId {
        if let Some(fill) = &self.fill {
            return fill.id();
        }

        let handle = ctx.ui.ctx().load_texture(
            "segments-fill",
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

    /// Screen span of the loop, from the loop point to the release point or the end.
    fn loop_region(&self, ctx: &Ctx<'_>) -> Option<Rect> {
        if !self.markers {
            return None;
        }

        let start = ctx.segs.loop_point()?;
        let end_x = ctx.segs.release_point().map_or(1.0, |point| point.x);
        let left = ctx.screen_x(start.x);
        let right = ctx.screen_x(end_x);

        (right > left).then(|| Rect::from_x_y_ranges(left..=right, ctx.plot.y_range()))
    }

    fn paint_point(&self, ctx: &Ctx<'_>, index: usize, hover: Option<Handle>) {
        let point = ctx.segs.point(index);
        let center = ctx.screen_pos(curve_point(point));
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
        let center = ctx.screen_pos(curve);

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

    /// Letter and color of the loop or release point. Never both: the loop sits before the release.
    fn marker(&self, ctx: &Ctx<'_>, index: usize) -> Option<(&'static str, Color32)> {
        if !self.markers {
            None
        } else if ctx.segs.loop_point_idx() == Some(index) {
            Some(("L", C::LOOP_COLOR))
        } else if ctx.segs.release_point_idx() == Some(index) {
            Some(("R", C::RELEASE_COLOR))
        } else {
            None
        }
    }

    /// Hover ring, and a filled disc while dragging. Drawn on the handle layer.
    /// A hovered or dragged breakpoint shows its phase and level.
    fn paint_handle(
        &self,
        ctx: &Ctx<'_>,
        center: Pos2,
        radius: f32,
        handle: Handle,
        hover: Option<Handle>,
    ) {
        if ctx.stamping {
            return;
        }

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
            let phase = Units::Normalized.format(point.x.clamp(0.0, 1.0));
            let level = Units::Normalized.format(point.y.clamp(0.0, 1.0));

            Self::paint_level(ctx, center, format!("{phase}  {level}"));
        }

        painter.circle_stroke(center, radius, Stroke::new(1.0, Color32::WHITE));
    }

    /// Readout next to `anchor`, kept inside the clip rect.
    fn paint_level(ctx: &Ctx<'_>, anchor: Pos2, text: String) {
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

    /// Left stays the curve color. The right channel keeps that saturation and value.
    fn paint_playheads(&self, ctx: &Ctx<'_>, playhead: [(Smp, Smp); 2]) {
        let painter = ctx.ui.painter();
        let [left, right] = playhead.map(|(phase, level)| ctx.playhead_pos(phase, level));

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
