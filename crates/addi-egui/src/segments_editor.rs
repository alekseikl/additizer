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
use utils::{
    Ctx, Drag, Handle, Pattern, TOOLS, Tool, bold, curve_point, snap_near, stack_y, with_alpha,
};

mod consts;
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
        let painting = self.painting();

        if plot.is_positive() {
            self.plot(
                &mut Ctx {
                    ui,
                    plot,
                    res: &response,
                    painting,
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

    fn painting(&self) -> bool {
        matches!(self.tool, Tool::Paint(_))
    }

    fn plot(&mut self, ctx: &mut Ctx<'_>, playhead: Option<[(Smp, Smp); 2]>) {
        let mut remove = None;

        if !ctx.ui.input(|input| input.pointer.primary_down()) {
            self.drag = None;
        }

        if ctx.painting {
            self.drag = None;
            self.paint_pointer(ctx);
            Self::canvas_menu(ctx);
        } else {
            self.paint_column = None;
            Self::interact_canvas(ctx);

            let points = ctx.segs.num_points();

            for index in 0..points {
                if self.interact_point(ctx, index) {
                    remove = Some(index);
                }

                self.interact_power_point(ctx, index);
            }
        }

        self.paint_scene(ctx);

        if ctx.painting && (ctx.res.hovered() || ctx.res.is_pointer_button_down_on()) {
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
        Self::canvas_menu(ctx);

        if ctx.res.double_clicked_by(PointerButton::Primary)
            && let Some(pos) = ctx.res.interact_pointer_pos().or(ctx.res.hover_pos())
        {
            ctx.add_point(ctx.curve_pos(pos));
        }
    }

    /// One power point: drag, double-click, and menu.
    fn interact_power_point(&mut self, ctx: &mut Ctx<'_>, index: usize) {
        if !self.power_point_active(ctx, index) {
            return;
        }

        let center = ctx.screen_pos(self.power_position(ctx, index));
        let response = ctx.handle_response(center, C::POWER_GRAB_RADIUS, ("segments-power", index));

        Self::power_menu(ctx, &response, index);

        if response.double_clicked_by(PointerButton::Primary) {
            ctx.segs.set_power(index, 0.0);
            self.drag = None;
        } else if let Some(pos) = self.drag_begin(ctx, &response) {
            self.drag = Some(Drag::new(
                ctx.curve_pos(pos),
                self.power_position(ctx, index),
                Handle::Power(index),
            ));
            self.drag_power = ctx.segs.power(index);
        }

        if let Some((_, delta)) = self.drag_move(ctx, Handle::Power(index)) {
            self.move_power(ctx, index, delta.y);
        }
    }

    /// One breakpoint: drag, double-click, and menu.
    fn interact_point(&mut self, ctx: &mut Ctx<'_>, index: usize) -> bool {
        let center = ctx.screen_pos(curve_point(ctx.segs.point(index)));
        let response = ctx.handle_response(center, C::GRAB_RADIUS, ("segments-point", index));
        let mut remove = self.point_menu(ctx, &response, index);

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

    /// Shift places the point freely.
    fn move_point(&self, ctx: &mut Ctx<'_>, index: usize, x: Smp, y: Smp) {
        if index >= ctx.segs.num_points() {
            return;
        }

        let (min_x, max_x) = ctx.x_limits(index);
        let mut x = x.clamp(min_x, max_x);
        let mut y = y.clamp(0.0, 1.0);

        if !ctx.shift_down() {
            x = snap_near(x, self.grid_x, C::SNAP_X, min_x, max_x);
            y = snap_near(y, self.grid_y, C::SNAP_Y, 0.0, 1.0);
        }

        self.match_wrapped_ends(ctx, index, y);

        let current = ctx.segs.point(index);
        ctx.segs.set_point(index, SegPoint { x, y, ..current });
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

        if self.segment_end(ctx, next).is_some() && ctx.segs.power(next) != -power {
            ctx.segs.set_power(next, -power);
        }
    }

    /// `true` when the menu asked to remove this point. Other choices edit the curve directly.
    fn point_menu(&self, ctx: &mut Ctx<'_>, response: &Response, index: usize) -> bool {
        let mut remove = false;

        response.context_menu(|ui| {
            if index >= ctx.segs.num_points() {
                return;
            }

            Self::smooth_item(ui, ctx.segs, index);

            if self.markers {
                Self::marker_items(ui, ctx.segs, index);
            }

            if ctx.is_inner(index) && ui.button("Remove point").clicked() {
                remove = true;
                ui.close();
            }
        });

        remove
    }

    fn power_menu(ctx: &mut Ctx<'_>, response: &Response, index: usize) {
        response.context_menu(|ui| {
            if index < ctx.segs.num_points() {
                Self::smooth_item(ui, ctx.segs, index);
            }

            if ui.button("Reset bend").clicked() {
                ctx.segs.set_power(index, 0.0);
                ui.close();
            }
        });
    }

    fn canvas_menu(ctx: &mut Ctx<'_>) {
        ctx.res.context_menu(|ui| {
            if ui.button("Flip horizontal").clicked() {
                ctx.segs.flip_horizontal();
                ui.close();
            }

            if ui.button("Flip vertical").clicked() {
                ctx.segs.flip_vertical();
                ui.close();
            }

            if ui.button("Reset bends").clicked() {
                ctx.segs.reset_powers();
                ui.close();
            }
        });
    }

    /// Toggles the sine easing of segment `index`.
    fn smooth_item(ui: &mut Ui, segments: &mut Segments, index: usize) {
        let smooth = segments.point(index).smooth;
        let label = if smooth { "Flat" } else { "Smooth" };

        if ui.button(label).on_hover_text(C::SMOOTH_TIP).clicked() {
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

            if can_loop && ui.button("Loop point").on_hover_text(C::LOOP_TIP).clicked() {
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
                    .on_hover_text(C::RELEASE_TIP)
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
        self.tool = if self.tool == tool { Tool::Edit } else { tool };
    }

    fn tool_button(&self, ui: &mut Ui, tool: Tool) -> Response {
        let (rect, response) = ui.allocate_exact_size(C::TOOL_BUTTON, Sense::click());
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
        let points = C::EDIT_ICON.map(|(x, y)| {
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
                with_alpha(color, 0x55),
                Stroke::new(1.0, color),
            ));
        }
    }

    /// Highlights the column under the pointer and stamps into it while the button is held.
    fn paint_pointer(&mut self, ctx: &mut Ctx<'_>) {
        let pos = ctx.res.interact_pointer_pos().or(ctx.res.hover_pos());

        self.paint_column = pos.map(|pos| self.column_at(ctx, pos));

        let primary = ctx
            .ui
            .input(|input| input.pointer.button_down(PointerButton::Primary));
        let down = ctx.res.is_pointer_button_down_on();
        let pos = ctx.res.interact_pointer_pos();

        if primary
            && down
            && let Some(pos) = pos
        {
            self.paint_at(ctx, pos);
        }
    }

    /// Shift places the level freely. Otherwise it snaps to the vertical grid.
    fn paint_at(&self, ctx: &mut Ctx<'_>, pos: Pos2) {
        let Tool::Paint(pattern) = self.tool else {
            return;
        };

        let mut level = ctx.curve_pos(pos).y.clamp(0.0, 1.0);

        if !ctx.shift_down() {
            level = snap_near(level, self.grid_y, C::SNAP_Y, 0.0, 1.0);
        }

        let (from_x, to_x) = self.column_bounds(self.column_at(ctx, pos));

        Self::paint_cell(ctx, from_x, to_x, level, pattern);
    }

    fn column_at(&self, ctx: &Ctx<'_>, pos: Pos2) -> i32 {
        let columns = self.grid_x.max(1);
        let x = ctx.curve_pos(pos).x.clamp(0.0, 1.0);
        let column = (x * columns as Smp).floor() as i32;

        column.clamp(0, columns - 1)
    }

    fn column_bounds(&self, column: i32) -> (Smp, Smp) {
        let columns = self.grid_x.max(1) as Smp;
        let column = column.clamp(0, self.grid_x.max(1) - 1) as Smp;

        (column / columns, (column + 1.0) / columns)
    }

    /// Stamps `pattern` into `[from_x, to_x]`.
    fn paint_cell(ctx: &mut Ctx<'_>, from_x: Smp, to_x: Smp, level: Smp, pattern: Pattern) {
        let segments = &mut *ctx.segs;

        if !(from_x >= 0.0 && from_x < to_x && to_x <= 1.0) {
            return;
        }

        let level = level.clamp(0.0, 1.0);
        let n = segments.num_points();
        let left = (0..n).find(|&index| segments.point(index).x == from_x);
        let right = (0..n).rev().find(|&index| segments.point(index).x == to_x);
        let pattern_points = pattern.scaled(from_x, to_x, level);
        let mut stamped = Vec::with_capacity(n + pattern_points.len() + 2);

        for index in 0..n {
            let point = segments.point(index);

            if point.x < from_x || left == Some(index) {
                stamped.push(point);
            }
        }

        let insert_at = stamped.len();

        if left.is_none() {
            stamped.push(SegPoint {
                x: from_x,
                y: segments.value_at(from_x, None, None),
                ..SegPoint::default()
            });
        }

        stamped.extend(pattern_points);

        if right.is_none() {
            stamped.push(SegPoint {
                x: to_x,
                y: segments.value_at(to_x, None, None),
                ..SegPoint::default()
            });
        }

        let tail_at = stamped.len();

        for index in 0..n {
            let point = segments.point(index);

            if point.x > to_x || right == Some(index) {
                stamped.push(point);
            }
        }

        if stamped.len() > MAX_POINTS || segments.points() == stamped.as_slice() {
            return;
        }

        for index in (0..n).rev() {
            let on_left = segments.point(index).x < from_x || left == Some(index);
            let on_right = segments.point(index).x > to_x || right == Some(index);

            if !on_left && !on_right {
                assert!(segments.remove_point(index));
            }
        }

        for (offset, point) in stamped[insert_at..tail_at].iter().enumerate() {
            assert!(segments.add_point(insert_at + offset, *point));
        }

        debug_assert_eq!(segments.points(), stamped.as_slice());
    }

    /// A power point needs a sloped segment that is wide enough to show it.
    fn power_point_active(&self, ctx: &Ctx<'_>, index: usize) -> bool {
        let Some(end) = self.segment_end(ctx, index) else {
            return false;
        };
        let from = ctx.segs.point(index);

        ctx.plot.width() * (end.x - from.x) >= C::MIN_POWER_WIDTH && end.y != from.y
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

    /// Midpoint in phase, power-scaled in level. A wrap past 1 folds back into the plot.
    fn power_position(&self, ctx: &Ctx<'_>, index: usize) -> Pos2 {
        let from = curve_point(ctx.segs.point(index));
        let Some(end) = self.segment_end(ctx, index) else {
            return from;
        };
        let x = (from.x + end.x) * 0.5;
        let x = if x >= 1.0 { x - 1.0 } else { x };
        let t = power_scale(0.5, ctx.segs.power(index));

        pos2(x, from.y + t * (end.y - from.y))
    }

    /// Polyline with one sample per horizontal pixel and every breakpoint kept.
    fn build_curve_points(&mut self, ctx: &Ctx<'_>) {
        let plot = ctx.plot;
        let segments = &*ctx.segs;

        assert!(plot.width() > 0.0);

        let revision = segments.revision();
        let size = plot.size();

        if self.curve_cache != Some((revision, size)) {
            let steps = size.x.round().max(1.0) as usize;

            segments.render_points(steps, &mut self.curve, &mut self.scratch);
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

        for index in 0..ctx.segs.num_points() {
            self.paint_point(ctx, index);
            self.paint_power_point(ctx, index);
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

    fn paint_point(&self, ctx: &Ctx<'_>, index: usize) {
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

        self.paint_handle(ctx, center, C::GRAB_RADIUS, Handle::Point(index));
    }

    fn paint_power_point(&self, ctx: &Ctx<'_>, index: usize) {
        if !self.power_point_active(ctx, index) {
            return;
        }

        let center = ctx.screen_pos(self.power_position(ctx, index));

        ctx.ui
            .painter()
            .circle_filled(center, C::POWER_RADIUS, self.power_color);
        self.paint_handle(ctx, center, C::POWER_GRAB_RADIUS, Handle::Power(index));
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
    /// A dragged breakpoint also shows its level.
    fn paint_handle(&self, ctx: &Ctx<'_>, center: Pos2, radius: f32, handle: Handle) {
        if ctx.painting {
            return;
        }

        let dragging = self.dragged() == Some(handle);
        let pointer = ctx.ui.input(|input| input.pointer.hover_pos());
        let hovered =
            self.drag.is_none() && pointer.is_some_and(|pos| pos.distance(center) <= radius);

        if !dragging && !hovered {
            return;
        }

        let painter = ctx.ui.painter().clone().with_layer_id(ctx.handle_layer());

        if dragging {
            ctx.ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
            painter.circle_filled(center, radius, Color32::from_white_alpha(0x40));

            if let Handle::Point(index) = handle {
                let level = Units::Normalized.format(ctx.segs.point(index).y.clamp(0.0, 1.0));

                Self::paint_level(ctx, center, level);
            }
        }

        if hovered {
            ctx.ui.ctx().set_cursor_icon(CursorIcon::Grab);
        }

        painter.circle_stroke(center, radius, Stroke::new(1.0, Color32::WHITE));
    }

    /// Level readout next to the dragged point, kept inside the clip rect.
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
