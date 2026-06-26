use egui::{
    Color32, Painter, Pos2, Rect, ScrollArea, Sense, Shape, Ui, Vec2,
    epaint::{CubicBezierShape, PathStroke},
    scroll_area::ScrollSource,
    vec2,
};
use rustc_hash::FxHashMap;

use crate::{
    editor::grid::grid_widget::GridWidget,
    synth_engine::{
        DataType, ModuleId,
        ui_bridge::{GridVec, UiBridge, routing_state::ModuleIo},
    },
};

mod grid_widget;

const GRID_CELL_SIZE: f32 = 40.0;
const C_GRID: Color32 = Color32::from_rgb(52, 52, 52);
const GRID_T: f32 = 1.0;
const WIRE_T: f32 = 2.0;
const WIRE_MOD_T: f32 = 1.0;
/// Minimum horizontal offset of a wire's Bézier control points. It makes the
/// curve leave an output heading right and enter an input from the left, which
/// keeps it clear of the widgets it attaches to.
const WIRE_CTRL_MIN: f32 = 8.0;
const C_WIRE_PREVIEW: Color32 = Color32::from_rgb(180, 180, 180);

/// Compensates for egui-baseview negating horizontal wheel delta on macOS.
#[cfg(target_os = "macos")]
const TRACKPAD_SCROLL_MULTIPLIER: egui::Vec2 = vec2(-1.0, 1.0);
#[cfg(not(target_os = "macos"))]
const TRACKPAD_SCROLL_MULTIPLIER: egui::Vec2 = egui::Vec2::splat(1.0);

impl From<GridVec> for Vec2 {
    fn from(grid: GridVec) -> Self {
        vec2(grid.x as f32, grid.y as f32) * GRID_CELL_SIZE
    }
}

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

struct WireDragState {
    src_id: ModuleId,
    src_output_type: DataType,
    start_pos: Pos2,
    color: Color32,
    dropped_at: Option<u64>,
}

#[derive(Default)]
struct WidgetsState {
    wire_drag: Option<WireDragState>,
}

struct WidgetCtx<'a> {
    bridge: &'a mut UiBridge,
    state: &'a mut WidgetsState,
    moved_module_id: Option<ModuleId>,
}

pub struct Grid {
    widgets: Vec<GridWidget>,
    widgets_state: WidgetsState,
    /// Cached scrollable content size. It only grows while a widget is being
    /// dragged (so the scrollbars don't jitter mid-drag) and is recomputed
    /// from scratch once the drag ends.
    content_size: egui::Vec2,
}

impl Grid {
    pub fn new() -> Self {
        Self {
            widgets: Vec::new(),
            widgets_state: WidgetsState::default(),
            content_size: egui::Vec2::ZERO,
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
        // Size the canvas to the widgets so the scrollbars track real content.
        // While dragging, only grow it so the scrollbars stay stable; recompute
        // it (allowing it to shrink) once the drag has ended. Keep half a
        // viewport of free space past the bottom-right-most widget.
        let extent = self.content_extent(bridge) + ui.available_size() * 0.5;
        let dragging = self.widgets.iter().any(GridWidget::is_dragging)
            || self.widgets_state.wire_drag.is_some();

        self.content_size = if dragging {
            self.content_size.max(extent)
        } else {
            extent
        };

        // Never smaller than the viewport so the grid fills the panel.
        let content_size = self.content_size.max(ui.available_size());

        ScrollArea::both()
            .scroll_source(ScrollSource {
                drag: false,
                ..Default::default()
            })
            .wheel_scroll_multiplier(TRACKPAD_SCROLL_MULTIPLIER)
            .auto_shrink([true, true])
            .show(ui, |ui| {
                let (response, painter) = ui.allocate_painter(content_size, Sense::hover());
                let canvas = response.rect;

                painter.rect_filled(canvas, 0.0, Color32::BLACK);
                Self::paint_grid(&painter, painter.clip_rect(), canvas.min);

                // Reserve a paint slot for the wires up front so they render
                // behind the modules, but fill it only after the widgets have
                // been drawn so it uses their up-to-date (current-frame) attach
                // points. This avoids the one-frame lag while dragging.
                let wires = painter.add(Shape::Noop);

                let mut ctx = WidgetCtx {
                    bridge,
                    state: &mut self.widgets_state,
                    moved_module_id: None,
                };

                for widget in &mut self.widgets {
                    widget.ui(ui, &mut ctx);
                }

                let moved_module_id = ctx.moved_module_id;

                if let Some(drag) = self.widgets_state.wire_drag.as_mut()
                    && let Some(dropped_at) = drag.dropped_at
                    && dropped_at < ui.ctx().cumulative_frame_nr()
                {
                    self.widgets_state.wire_drag = None;
                }

                let wire_shapes = self.wire_shapes();

                if let Some(drag) = &self.widgets_state.wire_drag
                    && let Some(pointer) = ui.ctx().pointer_hover_pos()
                {
                    ui.painter().add(self.preview_wire_shape(drag, pointer));
                }

                painter.set(wires, Shape::Vec(wire_shapes));

                let _ = response;

                if let Some(anchor) = moved_module_id {
                    self.resolve_overlaps(anchor, bridge);
                }
            });
    }

    /// Bottom-right extent of all widgets in canvas pixels (including any
    /// in-progress drag), plus a padding margin. Drives the scrollable area.
    fn content_extent(&self, bridge: &UiBridge) -> egui::Vec2 {
        let mut extent = egui::Vec2::ZERO;

        for widget in &self.widgets {
            let pos = bridge.get_module_position(widget.module_id());
            let cell_extent = pos + widget.grid_size();
            let bottom_right = Vec2::from(cell_extent) + widget.drag_offset();

            extent = extent.max(bottom_right);
        }

        extent
    }

    fn preview_wire_shape(&self, drag: &WireDragState, pointer: Pos2) -> Shape {
        let src_pos = drag.start_pos;
        let dst_pos = pointer;
        let output_color = drag.color;
        let dx = ((dst_pos.x - src_pos.x).abs() * 0.5).max(WIRE_CTRL_MIN);
        let ctrl1 = src_pos + vec2(dx, 0.0);
        let ctrl2 = dst_pos - vec2(dx, 0.0);
        let stroke = PathStroke::new_uv(WIRE_T, move |_, pos| {
            Self::wire_color_at(pos, src_pos, dst_pos, output_color, C_WIRE_PREVIEW)
        })
        .middle();

        Shape::CubicBezier(CubicBezierShape::from_points_stroke(
            [src_pos, ctrl1, ctrl2, dst_pos],
            false,
            Color32::TRANSPARENT,
            stroke,
        ))
    }

    fn wire_color_at(
        pos: Pos2,
        src_pos: Pos2,
        dst_pos: Pos2,
        output_color: Color32,
        input_color: Color32,
    ) -> Color32 {
        let seg = dst_pos - src_pos;
        let len_sq = seg.length_sq();
        let t = if len_sq > 0.0 {
            (pos - src_pos).dot(seg) / len_sq
        } else {
            0.0
        };
        let blend = ((t.clamp(0.0, 1.0) - 0.75) / 0.25).clamp(0.0, 1.0);
        output_color.lerp_to_gamma(input_color, blend)
    }

    /// Builds straight wire shapes from each module's output to the inputs it
    /// feeds, using the attach points captured during the current frame.
    fn wire_shapes(&self) -> Vec<Shape> {
        let outputs: FxHashMap<ModuleId, (Pos2, Color32)> = self
            .widgets
            .iter()
            .filter_map(|widget| {
                widget
                    .output_point()
                    .map(|anchor| (widget.module_id(), anchor))
            })
            .collect();

        let mut shapes = Vec::new();

        for widget in &self.widgets {
            for input in widget.input_points() {
                if let Some(&(src_pos, output_color)) = outputs.get(&input.module_id) {
                    let dst_pos = input.point;
                    let input_color = input.color;
                    let dx = ((dst_pos.x - src_pos.x).abs() * 0.5).max(WIRE_CTRL_MIN);
                    let ctrl1 = src_pos + vec2(dx, 0.0);
                    let ctrl2 = dst_pos - vec2(dx, 0.0);
                    let thickness = if input.is_modulation {
                        WIRE_MOD_T
                    } else {
                        WIRE_T
                    };
                    let stroke = PathStroke::new_uv(thickness, move |_, pos| {
                        Self::wire_color_at(pos, src_pos, dst_pos, output_color, input_color)
                    })
                    .middle();

                    shapes.push(Shape::CubicBezier(CubicBezierShape::from_points_stroke(
                        [src_pos, ctrl1, ctrl2, dst_pos],
                        false,
                        Color32::TRANSPARENT,
                        stroke,
                    )));
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
                let GridVec { x, y } = bridge.get_module_position(id);
                let GridVec { x: w, y: h } = widget.grid_size();

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
                bridge.set_module_position(
                    rect.id,
                    GridVec {
                        x: rect.x,
                        y: rect.y,
                    },
                );
            }

            settled.push(rect);
        }
    }

    fn paint_grid(painter: &Painter, area: Rect, origin: Pos2) {
        let stroke = PathStroke::new(GRID_T, C_GRID).inside();

        let x0 = origin.x + ((area.left() - origin.x) / GRID_CELL_SIZE).floor() * GRID_CELL_SIZE;
        let mut x = x0;
        while x <= area.right() {
            painter.line(
                vec![Pos2::new(x, area.top()), Pos2::new(x, area.bottom())],
                stroke.clone(),
            );
            x += GRID_CELL_SIZE;
        }

        let y0 = origin.y + ((area.top() - origin.y) / GRID_CELL_SIZE).floor() * GRID_CELL_SIZE;
        let mut y = y0;
        while y <= area.bottom() {
            painter.line(
                vec![Pos2::new(area.left(), y), Pos2::new(area.right(), y)],
                stroke.clone(),
            );
            y += GRID_CELL_SIZE;
        }
    }
}
