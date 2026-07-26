use egui::{
    Align, Color32, Id, Label, LayerId, Layout, Order, PointerButton, Pos2, Rect, Response, Sense,
    Stroke, Ui, UiBuilder, Vec2,
    emath::{self, GuiRounding},
    lerp, vec2,
};

use crate::{
    editor::grid::{
        GridEvent, WidgetCtx, WireDragState,
        input_mixer_popup::InputMixerPopup,
        grid_widget::{
            amplifier_widget::AmplifierWidget, envelope_widget::EnvelopeWidget,
            harmonic_editor_widget::HarmonicEditorWidget, lfo_widget::LfoWidget,
            mixer_widget::MixerWidget, oscillator_widget::OscillatorWidget,
            output_widget::OutputWidget, spectral_blend_widget::SpectralBlendWidget,
            spectral_filter_widget::SpectralFilterWidget,
            spectral_mixer_widget::SpectralMixerWidget,
        },
        input_tooltip,
        select_input_popup::SelectInputPopup,
    },
    synth_engine::{
        DataType, Input, InputId, InputSource, ModuleId, ModuleType,
        ui_bridge::{
            GridVec,
            routing_state::{ModuleInput, ModuleIo},
        },
    },
};

mod amplifier_widget;
mod envelope_widget;
mod harmonic_editor_widget;
mod lfo_widget;
mod mixer_widget;
mod oscillator_widget;
mod output_widget;
mod spectral_blend_widget;
mod spectral_filter_widget;
mod spectral_mixer_widget;

const C_MOD_BG: Color32 = Color32::from_rgb(28, 30, 42);
const C_MOD_BG_SELECTED: Color32 = Color32::from_rgb(40, 42, 54);
const CORNER_RADIUS: f32 = 4.0;
const BLOCK_MARGIN: f32 = 3.0;

const IO_STRIPE_W: f32 = 16.0;
const INPUTS_PADDING: f32 = 4.0;
const INPUTS_PER_CELL: i32 = 2;
const IO_SLOT_H: f32 = 16.0;
const IO_DOT_SIZE: f32 = 8.0;
const IO_DOT_SIZE_HOVER: f32 = 10.0;
const WIRE_THICKNESS: f32 = 2.0;
const C_INPUT_STRIPE_HOVER: Color32 = Color32::from_rgb(52, 54, 68);

pub trait GridWidgetContent: Send {
    fn grid_size(&self) -> GridVec {
        GridVec::new(4, 2)
    }

    fn show_label(&self) -> bool {
        true
    }

    fn ui(&mut self, ui: &mut Ui, ctx: &mut WidgetCtx, module_id: ModuleId);
}

pub struct EmptyContent {}

impl GridWidgetContent for EmptyContent {
    fn ui(&mut self, _ui: &mut Ui, _ctx: &mut WidgetCtx, _module_id: ModuleId) {}
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

struct EditInputRequest {
    input: Input,
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
    edit_input: Option<EditInputRequest>,
}

impl GridWidget {
    pub fn new(io: ModuleIo) -> Self {
        let module_type = io.module_type;

        Self {
            io,
            content: match module_type {
                ModuleType::Oscillator => Box::new(OscillatorWidget::default()),
                ModuleType::Amplifier => Box::new(AmplifierWidget::default()),
                ModuleType::Mixer => Box::new(MixerWidget::default()),
                ModuleType::SpectralFilter => Box::new(SpectralFilterWidget {}),
                ModuleType::Envelope => Box::new(EnvelopeWidget {}),
                ModuleType::Lfo => Box::new(LfoWidget::default()),
                ModuleType::HarmonicEditor => Box::new(HarmonicEditorWidget {}),
                ModuleType::SpectralBlend => Box::new(SpectralBlendWidget {}),
                ModuleType::SpectralMixer => Box::new(SpectralMixerWidget {}),
                ModuleType::Output => Box::new(OutputWidget::default()),
                _ => Box::new(EmptyContent {}),
            },
            drag_offset: Vec2::ZERO,
            drag_grab: None,
            output_pos: None,
            input_positions: Vec::new(),
            link_request: None,
            edit_input: None,
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
                let mut points = Vec::new();

                match &input.sources {
                    InputSource::Direct(module_id) => {
                        points.push(InputPoint {
                            module_id: *module_id,
                            point,
                            color,
                            is_modulation: false,
                        });
                    }
                    InputSource::Mixed(sources) => {
                        for source in sources {
                            points.push(InputPoint {
                                module_id: source.module_id,
                                point,
                                color,
                                is_modulation: false,
                            });

                            if let Some(module_id) = source.modulation {
                                points.push(InputPoint {
                                    module_id,
                                    point,
                                    color,
                                    is_modulation: true,
                                });
                            }
                        }
                    }
                }

                points
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
            ctx.events.push(GridEvent::Moved(self.io.id));
        }

        self.link_request_ui(ui, ctx);
        self.edit_input_ui(ui, ctx);
    }

    fn main_ui(&mut self, ui: &mut Ui, ui_builder: UiBuilder, ctx: &mut WidgetCtx) -> Response {
        ui.scope_builder(ui_builder, |ui| {
            let full_width = ui.available_width();
            let full_height = ui.available_height();

            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            let bg = if ctx.selected_module_id == Some(self.io.id) {
                C_MOD_BG_SELECTED
            } else {
                C_MOD_BG
            };
            ui.painter().rect_filled(ui.max_rect(), CORNER_RADIUS, bg);

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

        let popup = SelectInputPopup {
            src: req.module_id,
            dst: self.io.id,
            pos: req.pos,
        };

        if popup.show(ui, ctx, self.io.module_type) {
            self.link_request = None;
        }
    }

    fn edit_input_ui(&mut self, ui: &mut Ui, ctx: &mut WidgetCtx) {
        let Some(req) = self.edit_input.as_ref() else {
            return;
        };

        let popup = InputMixerPopup {
            module_id: self.io.id,
            module_type: self.io.module_type,
            input: req.input,
            pos: req.pos,
        };

        if popup.show(ui, ctx) {
            self.edit_input = None;
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

    fn modulated_dot_color(ctx: &WidgetCtx, module_id: ModuleId, input: &ModuleInput) -> Color32 {
        let base = input.meta.input_type.color();

        if input.meta.data_type != DataType::Control {
            return base;
        }

        let blend = ctx
            .bridge
            .get_input_modulated_value(InputId::new(input.meta.input_type, module_id))
            .map(|modulated| {
                modulated
                    .normalized
                    .left()
                    .max(modulated.normalized.right())
            })
            .unwrap_or(0.0);

        base.lerp_to_gamma(Color32::WHITE, blend)
    }

    fn draw_input(
        &self,
        ui: &mut Ui,
        ctx: &mut WidgetCtx,
        height: f32,
        input: &ModuleInput,
    ) -> (Pos2, Option<EditInputRequest>) {
        let width = ui.available_width();
        let (rect, response) = ui.allocate_exact_size(vec2(width, height), Sense::click_and_drag());
        let wire_color = input.meta.input_type.color();
        let dot_color = Self::modulated_dot_color(ctx, self.io.id, input);
        let mut edit_request = None;

        if response.double_clicked_by(PointerButton::Primary) {
            ctx.bridge
                .remove_input_links(InputId::new(input.meta.input_type, self.io.id));
        } else if response.clicked_by(PointerButton::Primary) && !input.meta.is_direct {
            let input_id = InputId::new(input.meta.input_type, self.io.id);

            if !ctx.bridge.get_connected_input_sources(input_id).is_empty() {
                edit_request = Some(EditInputRequest {
                    input: input.meta.input_type,
                    pos: response.interact_pointer_pos().unwrap_or(rect.center()),
                });
            }
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
            Stroke::new(WIRE_THICKNESS, wire_color),
        );
        painter.circle_filled(center, dot_size * 0.5, dot_color);

        input_tooltip::show_above(
            ui,
            &response,
            center - vec2(0.0, dot_size * 0.5),
            self.io.module_type.input_label(input.meta.input_type),
        );

        (
            rect.left_center()
                .round_to_pixels(ui.ctx().pixels_per_point()),
            edit_request,
        )
    }

    fn draw_output(&self, ui: &mut Ui, ctx: &mut WidgetCtx, height: f32) -> (Pos2, Pos2, Response) {
        let width = ui.available_width();
        let (rect, response) = ui.allocate_exact_size(vec2(width, height), Sense::click_and_drag());
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

        if !ctx.bridge.has_linkable_input(drag.src_id, self.io.id) {
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
        let mut edit_request = None;

        for i in 0..self.io.inputs.len() {
            let (pos, request) = self.draw_input(ui, ctx, IO_SLOT_H, &self.io.inputs[i]);
            positions.push(pos);
            if request.is_some() {
                edit_request = request;
            }
        }

        self.input_positions = positions;

        if let Some(edit_request) = edit_request {
            self.edit_input = Some(edit_request);
        }
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

    fn content_ui(&mut self, ui: &mut Ui, ctx: &mut WidgetCtx) -> Response {
        let rect = ui.max_rect();
        let sense = if ctx.state.wire_drag.is_some() {
            Sense::hover()
        } else {
            Sense::click_and_drag()
        };
        let response = ui.interact(rect, ui.id().with(("drag-handle", self.io.id)), sense);

        if response.dragged() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        } else if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        }

        if response.clicked_by(PointerButton::Primary) {
            ctx.events.push(GridEvent::Selected(self.io.id));
        }

        if self.content.show_label() {
            ui.add_space(2.0);
            ui.add(
                Label::new(ctx.bridge.get_module_label(self.io.id))
                    .selectable(false)
                    .truncate(),
            );
            ui.add_space(2.0);
        }

        self.content.ui(ui, ctx, self.io.id);

        response
    }
}
