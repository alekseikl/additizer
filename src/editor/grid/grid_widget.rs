use rustc_hash::FxHasher;
use std::hash::{Hash, Hasher};

use egui::{
    Align, Color32, CornerRadius, Direction, Frame, Label, Layout, Rect, RichText, Sense, Stroke,
    StrokeKind, Ui, UiBuilder, Vec2, ecolor::Hsva, lerp, pos2, vec2,
};

use crate::{
    editor::grid::GRID_CELL_SIZE,
    synth_engine::{
        DataType, Input, ModuleId,
        ui_bridge::{
            UiBridge,
            routing_state::{ModuleInput, ModuleIo},
        },
    },
};

const C_MOD_BG: Color32 = Color32::from_rgb(28, 30, 42);
const C_INPUT_STRIPE: Color32 = Color32::from_rgb(24, 26, 36);
const C_INPUT_BG: Color32 = Color32::from_rgb(36, 38, 54);
const C_INPUT_BORDER: Color32 = Color32::from_rgb(76, 80, 118);
const C_INPUT_HOVER_BG: Color32 = Color32::from_rgb(52, 56, 78);
const C_INPUT_HOVER_BORDER: Color32 = Color32::from_rgb(138, 150, 200);
const C_INPUT_DOT: Color32 = Color32::from_rgb(178, 196, 242);
const CORNER_RADIUS: f32 = 4.0;
const INPUT_CORNER_RADIUS: f32 = 2.0;
const BLOCK_MARGIN: f32 = 1.0;
const INPUT_STRIPE_W: f32 = 16.0;
const INPUT_INDICATOR_SIZE: Vec2 = vec2(9.0, 7.0);
const INPUT_DOT_R: f32 = 2.5;

const IO_STRIPE_W: f32 = 16.0;
const INPUTS_PADDING: f32 = 4.0;
const INPUTS_PER_CELL: i32 = 4;
const IO_SLOT_H: f32 = 16.0;
const IO_DOT_SIZE: f32 = 8.0;
const IO_DOT_SIZE_HOVER: f32 = 10.0;
const WIRE_THICKNESS: f32 = 2.0;

pub trait GridWidgetContent: Send {
    fn grid_size(&self) -> (i32, i32);
    fn paint(&mut self);
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
            s: 0.25,
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
            s: 0.25,
            v: 0.5,
            a: 1.0,
        })
    }
}

pub struct EmptyContent {}

impl GridWidgetContent for EmptyContent {
    fn grid_size(&self) -> (i32, i32) {
        (2, 1)
    }

    fn paint(&mut self) {}
}

pub struct GridWidget {
    io: ModuleIo,
    content: Box<dyn GridWidgetContent>,
}

impl GridWidget {
    pub fn new(io: ModuleIo) -> Self {
        Self {
            io,
            content: Box::new(EmptyContent {}),
        }
    }

    pub fn module_id(&self) -> ModuleId {
        self.io.id
    }

    pub fn grid_size(&self) -> (i32, i32) {
        let (w, h) = self.content.grid_size();
        // Height required by inputs
        let inputs_h = (self.io.inputs.len() as i32 + INPUTS_PER_CELL - 1) / INPUTS_PER_CELL;

        (w, h.max(inputs_h))
    }

    pub fn update(&mut self, module_io: ModuleIo) {
        self.io = module_io;
    }

    pub fn ui(&mut self, ui: &mut Ui, bridge: &mut UiBridge) {
        let (gx, gy) = bridge.get_module_position(self.io.id);
        let (gw, gh) = self.grid_size();
        let size = vec2(gw as f32, gh as f32) * GRID_CELL_SIZE - Vec2::splat(1.0);
        let pos = vec2(gx as f32, gy as f32) * Vec2::splat(GRID_CELL_SIZE) + Vec2::splat(1.0);
        let max_rect = Rect::from_min_size(ui.min_rect().min + pos, size).shrink(BLOCK_MARGIN);

        let ui_builder = UiBuilder::new()
            .id_salt(("module-widget", self.io.id))
            .max_rect(max_rect)
            .layout(Layout::left_to_right(Align::Min));

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
                    self.inputs_ui(ui);
                },
            );

            ui.allocate_ui_with_layout(
                vec2(full_width - 2.0 * IO_STRIPE_W, full_height),
                Layout::top_down(Align::Center),
                |ui| {
                    self.content_ui(ui);
                },
            );

            ui.allocate_ui_with_layout(
                vec2(IO_STRIPE_W, full_height),
                Layout::top_down(Align::Center),
                |ui| {
                    self.output_ui(ui);
                },
            );
        });
    }

    fn draw_input(ui: &mut Ui, height: f32, input: &ModuleInput) {
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

        // painter.rect_filled(rect, 0.0, Color32::RED);

        painter.line_segment(
            [rect.left_center(), center],
            Stroke::new(WIRE_THICKNESS, color),
        );
        painter.circle_filled(center, dot_size * 0.5, color);
    }

    fn draw_output(ui: &mut Ui, height: f32, io: &ModuleIo) {
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

        let painter = ui.painter();

        if io.output_connected {
            let center = rect.center();
            painter.line_segment(
                [center, rect.right_center()],
                Stroke::new(WIRE_THICKNESS, color),
            );
            painter.circle_filled(center, radius, color);
        } else {
            let center = rect.center();
            painter.circle_stroke(
                center,
                radius - WIRE_THICKNESS * 0.5,
                Stroke::new(WIRE_THICKNESS, color),
            );
        }
    }

    fn inputs_ui(&self, ui: &mut Ui) {
        let full_height = ui.available_height();
        let inputs_count = self.io.inputs.len() as f32;
        let all_spaces = full_height - 2.0 * INPUTS_PADDING - inputs_count * IO_SLOT_H;
        let item_space = all_spaces / (inputs_count + 1.0);

        ui.set_min_width(IO_STRIPE_W);
        ui.spacing_mut().item_spacing = vec2(0.0, item_space);

        ui.add_space(INPUTS_PADDING + item_space);

        for input in self.io.inputs.iter() {
            Self::draw_input(ui, IO_SLOT_H, input);
        }
    }

    fn output_ui(&self, ui: &mut Ui) {
        ui.set_min_width(IO_STRIPE_W);

        let height = ui.available_height();
        let top = (height - IO_SLOT_H) * 0.5;
        ui.add_space(top);
        Self::draw_output(ui, IO_SLOT_H, &self.io);
    }

    fn content_ui(&self, ui: &mut Ui) {
        // ui.painter().rect_filled(ui.max_rect(), 0.0, Color32::RED);
        ui.label(self.io.module_type.label());
    }
}
