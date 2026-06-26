use rustc_hash::FxHasher;
use std::hash::{Hash, Hasher};

use egui::{
    Align, Color32, Id, Label, LayerId, Layout, Order, Pos2, Rect, Response, Sense, Stroke, Ui,
    UiBuilder, Vec2, ecolor::Hsva, emath::GuiRounding, lerp, vec2,
};

use crate::{
    editor::grid::GRID_CELL_SIZE,
    synth_engine::{
        DataType, Input, ModuleId, ModuleType,
        ui_bridge::{
            GridVec, UiBridge,
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

pub trait GridWidgetContent: Send {
    fn grid_size(&self) -> GridVec;
    fn ui(&mut self);
}

impl Input {
    /// Hue in [0.0, 1.0] for use with `Hsva::h`.
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

pub struct GridWidget {
    io: ModuleIo,
    content: Box<dyn GridWidgetContent>,
    /// Pixel offset applied while the widget is being dragged. Reset to zero
    /// once the drag finishes and the new grid position is committed.
    drag_offset: Vec2,
    /// Content-space point inside the widget that the pointer grabbed, relative
    /// to the widget's grid origin. `Some` only while a drag is in progress;
    /// tracking the pointer in content space keeps the widget under the cursor
    /// even as the canvas auto-scrolls.
    drag_grab: Option<Vec2>,
    /// Screen-space wire attach point at the widget's right edge, captured
    /// during the last frame. `None` for modules without an output (e.g.
    /// `Output`).
    output_pos: Option<Pos2>,
    /// Screen-space wire attach points at the widget's left edge, parallel to
    /// `io.inputs`.
    input_positions: Vec<Pos2>,
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
        }
    }

    /// Returns wire entry points for output
    pub fn output_point(&self) -> Option<(Pos2, Color32)> {
        self.output_pos
            .map(|pos| (pos, self.io.output_type.color()))
    }

    /// Returns wires entry points for inputs
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

    /// Pixel offset currently applied by an in-progress drag (zero otherwise).
    /// Lets the grid grow its scrollable content to follow a widget being
    /// dragged toward the edges.
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

    pub fn ui(&mut self, ui: &mut Ui, bridge: &mut UiBridge) -> Option<ModuleId> {
        let grid_pos = bridge.get_module_position(self.io.id);
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

        let drag = ui
            .scope_builder(ui_builder, |ui| {
                let full_width = ui.available_width();
                let full_height = ui.available_height();

                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                ui.painter()
                    .rect_filled(ui.max_rect(), CORNER_RADIUS, C_MOD_BG);

                ui.allocate_ui_with_layout(
                    vec2(IO_STRIPE_W, full_height),
                    Layout::top_down(Align::Center),
                    |ui| {
                        self.inputs_ui(ui);
                    },
                );

                let drag = ui
                    .allocate_ui_with_layout(
                        vec2(full_width - 2.0 * IO_STRIPE_W, full_height),
                        Layout::top_down(Align::Center),
                        |ui| self.content_ui(ui),
                    )
                    .inner;

                ui.allocate_ui_with_layout(
                    vec2(IO_STRIPE_W, full_height),
                    Layout::top_down(Align::Center),
                    |ui| {
                        self.output_ui(ui);
                    },
                );

                drag
            })
            .inner;

        self.handle_drag(ui, &drag, bridge, origin, pos, max_rect, grid_pos)
    }

    #[allow(clippy::too_many_arguments)]
    fn handle_drag(
        &mut self,
        ui: &mut Ui,
        drag: &Response,
        bridge: &mut UiBridge,
        origin: Pos2,
        base: Vec2,
        widget_rect: Rect,
        grid_pos: GridVec,
    ) -> Option<ModuleId> {
        if drag.drag_started() {
            // Record where inside the widget (in content space) the pointer
            // grabbed, so the cursor stays glued to that point during the drag.
            self.drag_grab = drag.interact_pointer_pos().map(|p| p - origin - base);
        }

        if drag.dragged()
            && let Some(grab) = self.drag_grab
            && let Some(pointer) = drag.interact_pointer_pos()
        {
            let offset = (pointer - origin) - base - grab;
            // Clamp so the widget can't be dragged past the top/left edges:
            // its grid origin must stay at or beyond (0, 0).

            self.drag_offset = offset.max(-Vec2::from(grid_pos));
            Self::auto_scroll(ui, widget_rect);
        }

        if !drag.drag_stopped() {
            return None;
        }

        let dx = (self.drag_offset.x / GRID_CELL_SIZE).round() as i32;
        let dy = (self.drag_offset.y / GRID_CELL_SIZE).round() as i32;
        let new_x = (grid_pos.x + dx).max(0);
        let new_y = (grid_pos.y + dy).max(0);

        bridge.set_module_position(self.io.id, GridVec { x: new_x, y: new_y });
        self.drag_offset = Vec2::ZERO;
        self.drag_grab = None;

        Some(self.io.id)
    }

    /// Scrolls the enclosing canvas when the dragged widget crosses a viewport
    /// edge, so it can be dragged into off-screen regions of the grid. Scroll
    /// speed grows with how far past the edge the widget reaches.
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

    /// Draws an input stub and returns the screen-space point at the widget's
    /// left edge where an incoming wire should attach.
    fn draw_input(ui: &mut Ui, height: f32, input: &ModuleInput) -> Pos2 {
        let width = ui.available_width();
        let (rect, response) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
        let color = input.meta.input_type.color();

        let t = ui.ctx().animate_bool_with_time_and_easing(
            response.id,
            response.hovered(),
            0.15,
            egui::emath::easing::cubic_out,
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

    /// Draws the output stub and returns the screen-space point at the widget's
    /// right edge where an outgoing wire should attach.
    fn draw_output(ui: &mut Ui, height: f32, io: &ModuleIo) -> Pos2 {
        let width = ui.available_width();
        let (rect, response) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
        let color = io.output_type.color();

        let t = ui.ctx().animate_bool_with_time_and_easing(
            response.id,
            response.hovered(),
            0.15,
            egui::emath::easing::cubic_out,
        );
        let dot_size = lerp(IO_DOT_SIZE..=IO_DOT_SIZE_HOVER, t);
        let radius = dot_size * 0.5;

        let center = rect.center();
        let painter = ui.painter();
        let ppt = ui.ctx().pixels_per_point();

        if io.output_connected {
            painter.line_segment(
                [center, rect.right_center()],
                Stroke::new(WIRE_THICKNESS, color),
            );
        }

        painter.circle_filled(center, radius, color);

        rect.right_center().round_to_pixels(ppt)
    }

    fn inputs_ui(&mut self, ui: &mut Ui) {
        let full_height = ui.available_height();
        let inputs_count = self.io.inputs.len() as f32;
        let all_spaces = full_height - 2.0 * INPUTS_PADDING - inputs_count * IO_SLOT_H;
        let item_space = all_spaces / (inputs_count + 1.0);

        ui.set_min_width(IO_STRIPE_W);
        ui.spacing_mut().item_spacing = vec2(0.0, item_space);

        ui.add_space(INPUTS_PADDING + item_space);

        let mut positions = Vec::with_capacity(self.io.inputs.len());
        for input in self.io.inputs.iter() {
            positions.push(Self::draw_input(ui, IO_SLOT_H, input));
        }
        self.input_positions = positions;
    }

    fn output_ui(&mut self, ui: &mut Ui) {
        ui.set_min_width(IO_STRIPE_W);

        if matches!(self.io.module_type, ModuleType::Output) {
            self.output_pos = None;
            return;
        }

        let height = ui.available_height();
        let top = (height - IO_SLOT_H) * 0.5;
        ui.add_space(top);
        self.output_pos = Some(Self::draw_output(ui, IO_SLOT_H, &self.io));
    }

    fn content_ui(&self, ui: &mut Ui) -> Response {
        let rect = ui.max_rect();
        let response = ui.interact(
            rect,
            ui.id().with(("drag-handle", self.io.id)),
            Sense::drag(),
        );

        if response.dragged() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        } else if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        }

        ui.add(Label::new(self.io.module_type.label()).selectable(false));

        response
    }
}
