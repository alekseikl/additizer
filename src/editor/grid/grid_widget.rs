use egui::{
    Align, Color32, CornerRadius, Direction, Frame, Label, Layout, Rect, RichText, Sense, Stroke,
    StrokeKind, Ui, UiBuilder, Vec2, pos2, vec2,
};

use crate::{
    editor::grid::GRID_CELL_SIZE,
    synth_engine::{
        ModuleId,
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

const INPUTS_PER_CELL: i32 = 4;

pub trait GridWidgetContent: Send {
    fn grid_size(&self) -> (i32, i32);
    fn paint(&mut self);
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
            ui.painter()
                .rect_filled(ui.max_rect(), CORNER_RADIUS, C_MOD_BG);

            ui.allocate_ui_with_layout(
                vec2(IO_STRIPE_W, ui.available_height()),
                Layout::top_down(Align::Center),
                |ui| {
                    self.inputs_ui(ui);
                },
            );
        });
    }

    fn inputs_ui(&self, ui: &mut Ui) {
        if self.io.inputs.len() == 0 {
            return;
        }

        let slot_h = ui.available_height() / (self.io.inputs.len() + 1) as f32;

        ui.set_min_width(IO_STRIPE_W);
        ui.set_max_width(IO_STRIPE_W);
        ui.spacing_mut().item_spacing = Vec2::ZERO;

        ui.painter()
            .rect_filled(ui.available_rect_before_wrap(), 0.0, C_INPUT_STRIPE);

        ui.add_space(0.5 * slot_h);

        for _ in self.io.inputs.iter() {
            ui.allocate_ui_with_layout(
                vec2(ui.available_width(), slot_h),
                Layout::centered_and_justified(Direction::TopDown),
                |ui| {
                    ui.add(Label::new(RichText::new("\u{23FA}").size(6.0)).selectable(false));
                },
            );
        }
    }
}
