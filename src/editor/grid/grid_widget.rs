use rustc_hash::FxHasher;
use std::hash::{Hash, Hasher};

use egui::{
    Align, Area, Color32, Id, Label, LayerId, Layout, Modal, Order, PointerButton, Pos2, Rect,
    Response, Sense, Stroke, Ui, UiBuilder, Vec2,
    ecolor::Hsva,
    emath::{self, GuiRounding},
    lerp, vec2,
};

use crate::{
    editor::grid::{WidgetCtx, WireDragState},
    synth_engine::{
        DataType, Input, InputId, ModuleId, ModuleType,
        ui_bridge::{
            GridVec,
            routing_state::{ModuleInput, ModuleIo},
        },
    },
};

const C_MOD_BG: Color32 = Color32::from_rgb(28, 30, 42);
const CORNER_RADIUS: f32 = 4.0;
const BLOCK_MARGIN: f32 = 3.0;

const IO_STRIPE_W: f32 = 16.0;
const INPUTS_PADDING: f32 = 4.0;
const INPUTS_PER_CELL: i32 = 4;
const IO_SLOT_H: f32 = 16.0;
const IO_DOT_SIZE: f32 = 8.0;
const IO_DOT_SIZE_HOVER: f32 = 10.0;
const WIRE_THICKNESS: f32 = 2.0;
const C_INPUT_STRIPE_HOVER: Color32 = Color32::from_rgb(40, 42, 54);

pub trait GridWidgetContent: Send {
    fn grid_size(&self) -> GridVec;
    fn ui(&mut self);
}

impl Input {
    fn hue(self) -> f32 {
        let mut hasher = FxHasher::default();

        self.hash(&mut hasher);
        hasher.finish() as f32 / u64::MAX as f32
    }

    fn color(self) -> Color32 {
        Color32::from(Hsva {
            h: self.hue(),
            s: 0.8,
            v: 0.5,
            a: 1.0,
        })
    }
}

impl DataType {
    fn color(self) -> Color32 {
        let h = match self {
            DataType::Audio => 0.58,
            DataType::Control => 0.36,
            DataType::Spectral => 0.84,
        };

        Color32::from(Hsva {
            h,
            s: 0.8,
            v: 0.5,
            a: 1.0,
        })
    }
}

pub struct EmptyContent {}

impl GridWidgetContent for EmptyContent {
    fn grid_size(&self) -> GridVec {
        GridVec::new(4, 2)
    }

    fn ui(&mut self) {}
}

pub struct OutputContent {}

impl GridWidgetContent for OutputContent {
    fn grid_size(&self) -> GridVec {
        GridVec { x: 2, y: 2 }
    }
    fn ui(&mut self) {}
}

pub struct InputPoint {
    pub module_id: ModuleId,
    pub point: Pos2,
    pub color: Color32,
    pub is_modulation: bool,
}

struct LinkRequest {
    module_id: ModuleId,
    pos: Pos2,
}

pub struct GridWidget {
    io: ModuleIo,
    content: Box<dyn GridWidgetContent>,
    // Widget's DnD offset
    drag_offset: Vec2,
    // DnD grab point within a widget in local widget coordinates
    drag_grab: Option<Vec2>,
    // Screen position of a wire output point
    output_pos: Option<Pos2>,
    // Screen positions of a wire input points
    input_positions: Vec<Pos2>,
    link_request: Option<LinkRequest>,
}

impl GridWidget {
    pub fn new(io: ModuleIo) -> Self {
        let module_type = io.module_type;

        Self {
            io,
            content: match module_type {
                ModuleType::Output => Box::new(OutputContent {}),
                _ => Box::new(EmptyContent {}),
            },
            drag_offset: Vec2::ZERO,
            drag_grab: None,
            output_pos: None,
            input_positions: Vec::new(),
            link_request: None,
        }
    }

    pub fn output_point(&self) -> Option<(Pos2, Color32)> {
        self.output_pos
            .map(|pos| (pos, self.io.output_type.color()))
    }

    pub fn input_points(&self) -> impl Iterator<Item = InputPoint> + '_ {
        self.io
            .inputs
            .iter()
            .zip(self.input_positions.iter())
            .flat_map(|(input, &point)| {
                let color = input.meta.input_type.color();

                input.sources.iter().flat_map(move |source| {
                    core::iter::once(InputPoint {
                        module_id: source.module_id,
                        point,
                        color,
                        is_modulation: false,
                    })
                    .chain(source.modulation.into_iter().map(
                        move |module_id| InputPoint {
                            module_id,
                            point,
                            color,
                            is_modulation: true,
                        },
                    ))
                })
            })
    }

    pub fn module_id(&self) -> ModuleId {
        self.io.id
    }

    pub fn is_dragging(&self) -> bool {
        self.drag_grab.is_some()
    }

    pub fn drag_offset(&self) -> Vec2 {
        self.drag_offset
    }

    pub fn grid_size(&self) -> GridVec {
        let size = self.content.grid_size();
        // Height required by inputs
        let inputs_h = (self.io.inputs.len() as i32 + INPUTS_PER_CELL - 1) / INPUTS_PER_CELL;

        GridVec {
            x: size.x,
            y: size.y.max(inputs_h),
        }
    }

    pub fn update(&mut self, module_io: ModuleIo) {
        self.io = module_io;
    }

    pub fn ui(&mut self, ui: &mut Ui, ctx: &mut WidgetCtx) {
        let grid_pos = ctx.bridge.get_module_position(self.io.id);
        let size = Vec2::from(self.grid_size()) - Vec2::splat(1.0);
        let pos = Vec2::from(grid_pos) + vec2(0.0, 1.0);
        let origin = ui.min_rect().min;
        let max_rect =
            Rect::from_min_size(origin + pos + self.drag_offset, size).shrink(BLOCK_MARGIN);

        let mut ui_builder = UiBuilder::new()
            .id(Id::new(("module-widget", self.io.id)))
            .max_rect(max_rect)
            .layout(Layout::left_to_right(Align::Min));

        if self.is_dragging() {
            ui_builder = ui_builder.layer_id(LayerId::new(
                Order::Foreground,
                Id::new(("dragged-module", self.io.id)),
            ));
        }

        let drag = self.main_ui(ui, ui_builder, ctx);

        if drag.drag_started() {
            self.drag_grab = drag.interact_pointer_pos().map(|p| p - origin - pos);
        }

        if drag.dragged()
            && let Some(grab) = self.drag_grab
            && let Some(pointer) = drag.interact_pointer_pos()
        {
            let offset = (pointer - origin) - pos - grab;
            // Clamp so the widget can't be dragged past the top/left edges:

            self.drag_offset = offset.max(-Vec2::from(grid_pos));
            Self::auto_scroll(ui, max_rect);
        }

        if drag.drag_stopped() {
            ctx.bridge.set_module_position(
                self.io.id,
                (grid_pos + GridVec::from_vec_rounded(self.drag_offset)).max(GridVec::ZERO),
            );
            self.drag_offset = Vec2::ZERO;
            self.drag_grab = None;
            ctx.moved_module_id = Some(self.io.id);
        }

        self.link_request_ui(ui, ctx);
    }

    fn main_ui(&mut self, ui: &mut Ui, ui_builder: UiBuilder, ctx: &mut WidgetCtx) -> Response {
        ui.scope_builder(ui_builder, |ui| {
            let full_width = ui.available_width();
            let full_height = ui.available_height();

            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            ui.painter()
                .rect_filled(ui.max_rect(), CORNER_RADIUS, C_MOD_BG);

            ui.allocate_ui_with_layout(
                vec2(IO_STRIPE_W, full_height),
                Layout::top_down(Align::Center),
                |ui| {
                    self.inputs_ui(ui, ctx);
                },
            );

            let drag = ui
                .allocate_ui_with_layout(
                    vec2(full_width - 2.0 * IO_STRIPE_W, full_height),
                    Layout::top_down(Align::Center),
                    |ui| self.content_ui(ui, ctx),
                )
                .inner;

            ui.allocate_ui_with_layout(
                vec2(IO_STRIPE_W, full_height),
                Layout::top_down(Align::Center),
                |ui| {
                    self.output_ui(ui, ctx);
                },
            );

            drag
        })
        .inner
    }

    fn link_request_ui(&mut self, ui: &mut Ui, ctx: &mut WidgetCtx) {
        let Some(req) = self.link_request.as_ref() else {
            return;
        };

        let inputs = ctx.bridge.get_connectable_inputs(req.module_id, self.io.id);

        if inputs.is_empty() {
            self.link_request = None;
            return;
        }

        let menu_id = Id::new("wire-link-menu");

        let modal = Modal::new(menu_id)
            .backdrop_color(Color32::TRANSPARENT)
            .area(
                Area::new(menu_id)
                    .fixed_pos(req.pos)
                    .order(Order::Foreground)
                    .kind(egui::UiKind::Popup),
            )
            .show(ui.ctx(), |ui| {
                ui.label("Connect to:");
                ui.separator();
                for input in &inputs {
                    if ui.button(format!("{:?}", input.meta.input_type)).clicked() {
                        ctx.bridge
                            .connect_source(req.module_id, input.input, input.meta);
                        ui.close();
                    }
                }
            });

        if modal.should_close() {
            self.link_request = None;
        }
    }

    fn auto_scroll(ui: &Ui, widget: Rect) {
        const MAX_SPEED: f32 = 18.0;

        let view = ui.clip_rect();
        let mut delta = Vec2::ZERO;

        let over_right = widget.right() - view.right();
        let over_left = view.left() - widget.left();
        let over_bottom = widget.bottom() - view.bottom();
        let over_top = view.top() - widget.top();

        if over_right > 0.0 {
            delta.x -= over_right.min(MAX_SPEED);
        } else if over_left > 0.0 {
            delta.x += over_left.min(MAX_SPEED);
        }

        if over_bottom > 0.0 {
            delta.y -= over_bottom.min(MAX_SPEED);
        } else if over_top > 0.0 {
            delta.y += over_top.min(MAX_SPEED);
        }

        if delta != Vec2::ZERO {
            ui.scroll_with_delta(delta);
            ui.ctx().request_repaint();
        }
    }

    fn draw_input(
        &self,
        ui: &mut Ui,
        ctx: &mut WidgetCtx,
        height: f32,
        input: &ModuleInput,
    ) -> Pos2 {
        let width = ui.available_width();
        let (rect, response) = ui.allocate_exact_size(vec2(width, height), Sense::click_and_drag());
        let color = input.meta.input_type.color();

        if response.double_clicked_by(PointerButton::Primary) {
            ctx.bridge
                .remove_input_links(InputId::new(input.meta.input_type, self.io.id));
        }

        let t = ui.ctx().animate_bool_with_time_and_easing(
            response.id,
            response.hovered() || response.dragged(),
            0.15,
            emath::easing::cubic_out,
        );
        let dot_size = lerp(IO_DOT_SIZE..=IO_DOT_SIZE_HOVER, t);

        let center = rect.center();
        let painter = ui.painter();

        painter.line_segment(
            [rect.left_center(), center],
            Stroke::new(WIRE_THICKNESS, color),
        );
        painter.circle_filled(center, dot_size * 0.5, color);

        rect.left_center()
            .round_to_pixels(ui.ctx().pixels_per_point())
    }

    fn draw_output(&self, ui: &mut Ui, ctx: &mut WidgetCtx, height: f32) -> (Pos2, Pos2, Response) {
        let width = ui.available_width();
        let (rect, hover) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
        let drag_id = hover.id.with("output-drag");
        let hit_rect = rect.expand(6.0);
        let response = ui.interact(hit_rect, drag_id, Sense::click_and_drag());
        let color = self.io.output_type.color();

        if response.double_clicked_by(PointerButton::Primary) {
            ctx.bridge.remove_output_links(self.io.id);
        }

        let t = ui.ctx().animate_bool_with_time_and_easing(
            response.id,
            response.hovered() || response.dragged(),
            0.15,
            egui::emath::easing::cubic_out,
        );
        let dot_size = lerp(IO_DOT_SIZE..=IO_DOT_SIZE_HOVER, t);
        let radius = dot_size * 0.5;

        let center = rect.center();
        let painter = ui.painter();
        let ppt = ui.ctx().pixels_per_point();

        if self.io.output_connected {
            painter.line_segment(
                [center, rect.right_center()],
                Stroke::new(WIRE_THICKNESS, color),
            );
        }

        painter.circle_filled(center, radius, color);

        if response.hovered() || response.dragged() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
        }

        (
            rect.right_center().round_to_pixels(ppt),
            rect.center().round_to_pixels(ppt),
            response,
        )
    }

    fn handle_inputs_dnd(&mut self, ui: &mut Ui, ctx: &mut WidgetCtx) {
        let Some(drag) = ctx.state.wire_drag.as_mut() else {
            return;
        };

        if drag.src_id == self.io.id || !self.io.has_compatible_input(drag.src_output_type) {
            return;
        }

        let stripe = ui.max_rect();

        ui.painter()
            .rect_filled(stripe, CORNER_RADIUS, C_INPUT_STRIPE_HOVER);

        if drag.dropped_at.is_some()
            && let Some(pointer) = ui.ctx().pointer_interact_pos()
            && stripe.contains(pointer)
        {
            self.link_request = Some(LinkRequest {
                module_id: drag.src_id,
                pos: pointer,
            });

            ctx.state.wire_drag = None;
        }
    }

    fn inputs_ui(&mut self, ui: &mut Ui, ctx: &mut WidgetCtx) {
        self.handle_inputs_dnd(ui, ctx);

        let full_height = ui.available_height();
        let inputs_count = self.io.inputs.len() as f32;
        let all_spaces = full_height - 2.0 * INPUTS_PADDING - inputs_count * IO_SLOT_H;
        let item_space = all_spaces / (inputs_count + 1.0);

        ui.set_min_width(IO_STRIPE_W);
        ui.spacing_mut().item_spacing = vec2(0.0, item_space);

        ui.add_space(INPUTS_PADDING + item_space);

        let mut positions = Vec::with_capacity(self.io.inputs.len());
        for input in self.io.inputs.iter() {
            positions.push(self.draw_input(ui, ctx, IO_SLOT_H, input));
        }
        self.input_positions = positions;
    }

    fn output_ui(&mut self, ui: &mut Ui, ctx: &mut WidgetCtx) {
        ui.set_min_width(IO_STRIPE_W);

        if matches!(self.io.module_type, ModuleType::Output) {
            self.output_pos = None;
            return;
        }

        let height = ui.available_height();
        let top = (height - IO_SLOT_H) * 0.5;

        ui.add_space(top);

        let (pos, center_pos, response) = self.draw_output(ui, ctx, IO_SLOT_H);
        self.output_pos = Some(pos);

        if response.drag_started() {
            ctx.state.wire_drag = Some(WireDragState {
                src_id: self.io.id,
                src_output_type: self.io.output_type,
                start_pos: center_pos,
                color: self.io.output_type.color(),
                dropped_at: None,
            });
        } else if let Some(drag) = ctx.state.wire_drag.as_mut()
            && drag.src_id == self.io.id
        {
            drag.start_pos = center_pos;
        }

        if response.dragged()
            && let Some(pointer) = ui.ctx().pointer_interact_pos()
        {
            Self::auto_scroll(ui, Rect::from_center_size(pointer, vec2(16.0, 16.0)));
        }

        if response.drag_stopped()
            && let Some(drag) = ctx.state.wire_drag.as_mut()
            && drag.src_id == self.io.id
        {
            drag.dropped_at = Some(ui.ctx().cumulative_frame_nr());
        }
    }

    fn content_ui(&self, ui: &mut Ui, ctx: &WidgetCtx) -> Response {
        let rect = ui.max_rect();
        let sense = if ctx.state.wire_drag.is_some() {
            Sense::hover()
        } else {
            Sense::drag()
        };
        let response = ui.interact(rect, ui.id().with(("drag-handle", self.io.id)), sense);

        if response.dragged() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        } else if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        }

        ui.add(Label::new(self.io.module_type.label()).selectable(false));

        response
    }
}
