use egui::{
    Color32, Painter, Pos2, Rect, ScrollArea, Sense, Stroke, Ui, Vec2, scroll_area::ScrollSource,
    vec2,
};
use rustc_hash::FxHashMap;

use crate::{
    editor::grid::grid_widget::GridWidget,
    synth_engine::{
        ModuleId,
        ui_bridge::{UiBridge, routing_state::ModuleIo},
    },
};

mod grid_widget;

const GRID_CELL_SIZE: f32 = 70.0;
const VIRTUAL_W: f32 = 4000.0;
const VIRTUAL_H: f32 = 3000.0;
const C_GRID: Color32 = Color32::from_rgb(52, 52, 52);
const GRID_T: f32 = 0.5;

pub struct Grid {
    widgets: Vec<GridWidget>,
}

impl Default for Grid {
    fn default() -> Self {
        Self::new()
    }
}

impl Grid {
    pub fn new() -> Self {
        Self {
            widgets: Vec::new(),
        }
    }

    pub fn update_widgets(&mut self, modules_io: FxHashMap<ModuleId, ModuleIo>) {
        let mut widgets_by_id: FxHashMap<ModuleId, GridWidget> =
            self.widgets.drain(..).map(|w| (w.module_id(), w)).collect();

        self.widgets = modules_io
            .into_iter()
            .map(|(id, module_io)| match widgets_by_id.remove(&id) {
                Some(mut widget) => {
                    widget.update(module_io);
                    widget
                }
                None => GridWidget::new(module_io),
            })
            .collect();
    }

    pub fn ui(&mut self, ui: &mut Ui, bridge: &mut UiBridge) {
        ScrollArea::both()
            .scroll_source(ScrollSource {
                drag: false,
                ..Default::default()
            })
            .auto_shrink([true, true])
            .show(ui, |ui| {
                let (response, painter) =
                    ui.allocate_painter(vec2(VIRTUAL_W, VIRTUAL_H), Sense::drag());

                let canvas = response.rect;
                painter.rect_filled(canvas, 0.0, Color32::BLACK);
                paint_grid(&painter, painter.clip_rect(), canvas.min);

                for widget in &mut self.widgets {
                    widget.ui(ui, bridge);
                }
            });
    }
}

fn paint_grid(painter: &Painter, area: Rect, origin: Pos2) {
    let stroke = Stroke::new(GRID_T, C_GRID);

    let x0 = origin.x + ((area.left() - origin.x) / GRID_CELL_SIZE).floor() * GRID_CELL_SIZE;
    let mut x = x0;
    while x <= area.right() {
        painter.line_segment(
            [Pos2::new(x, area.top()), Pos2::new(x, area.bottom())],
            stroke,
        );
        x += GRID_CELL_SIZE;
    }

    let y0 = origin.y + ((area.top() - origin.y) / GRID_CELL_SIZE).floor() * GRID_CELL_SIZE;
    let mut y = y0;
    while y <= area.bottom() {
        painter.line_segment(
            [Pos2::new(area.left(), y), Pos2::new(area.right(), y)],
            stroke,
        );
        y += GRID_CELL_SIZE;
    }
}
