use egui::{
    Color32, Painter, Pos2, Rect, ScrollArea, Sense, Shape, Stroke, Ui, scroll_area::ScrollSource,
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

const GRID_CELL_SIZE: f32 = 75.0;
const VIRTUAL_W: f32 = 4000.0;
const VIRTUAL_H: f32 = 3000.0;
const C_GRID: Color32 = Color32::from_rgb(52, 52, 52);
const GRID_T: f32 = 0.5;
const WIRE_T: f32 = 2.0;

struct GridRect {
    id: ModuleId,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
}

impl GridRect {
    fn overlaps(&self, other: &GridRect) -> bool {
        self.x < other.x + other.w
            && other.x < self.x + self.w
            && self.y < other.y + other.h
            && other.y < self.y + self.h
    }
}

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

                // Reserve a paint slot for the wires up front so they render
                // behind the modules, but fill it only after the widgets have
                // been drawn so it uses their up-to-date (current-frame) attach
                // points. This avoids the one-frame lag while dragging.
                let wires = painter.add(Shape::Noop);

                let mut dropped = None;

                for widget in &mut self.widgets {
                    if let Some(id) = widget.ui(ui, bridge) {
                        dropped = Some(id);
                    }
                }

                painter.set(wires, Shape::Vec(self.wire_shapes()));

                if let Some(anchor) = dropped {
                    self.resolve_overlaps(anchor, bridge);
                }
            });
    }

    /// Builds straight wire shapes from each module's output to the inputs it
    /// feeds, using the attach points captured during the current frame.
    fn wire_shapes(&self) -> Vec<Shape> {
        let outputs: FxHashMap<ModuleId, (Pos2, Color32)> = self
            .widgets
            .iter()
            .filter_map(|widget| widget.output_anchor().map(|anchor| (widget.module_id(), anchor)))
            .collect();

        let mut shapes = Vec::new();
        for widget in &self.widgets {
            for (src, dst_pos) in widget.input_connections() {
                if let Some(&(src_pos, color)) = outputs.get(&src) {
                    shapes.push(Shape::line_segment(
                        [src_pos, dst_pos],
                        Stroke::new(WIRE_T, color),
                    ));
                }
            }
        }

        shapes
    }

    /// After `anchor` was snapped to the grid, push every overlapping widget
    /// toward the bottom-right so no two widgets occupy the same cells. The
    /// anchor stays put; other widgets only ever move right or down.
    fn resolve_overlaps(&self, anchor: ModuleId, bridge: &mut UiBridge) {
        let mut rects: Vec<GridRect> = self
            .widgets
            .iter()
            .map(|widget| {
                let id = widget.module_id();
                let (x, y) = bridge.get_module_position(id);
                let (w, h) = widget.grid_size();

                GridRect { id, x, y, w, h }
            })
            .collect();

        // The anchor is fixed; settle it first. Remaining widgets are settled in
        // reading order (top-left first) so pushes cascade toward bottom-right.
        let mut settled: Vec<GridRect> = Vec::with_capacity(rects.len());
        if let Some(pos) = rects.iter().position(|r| r.id == anchor) {
            settled.push(rects.remove(pos));
        }
        rects.sort_by_key(|r| (r.y, r.x));

        for mut rect in rects {
            let original = (rect.x, rect.y);

            while let Some(blocker) = settled.iter().find(|s| s.overlaps(&rect)) {
                let push_right = blocker.x + blocker.w - rect.x;
                let push_down = blocker.y + blocker.h - rect.y;

                if push_right <= push_down {
                    rect.x = blocker.x + blocker.w;
                } else {
                    rect.y = blocker.y + blocker.h;
                }
            }

            if (rect.x, rect.y) != original {
                bridge.set_module_position(rect.id, rect.x, rect.y);
            }

            settled.push(rect);
        }
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
