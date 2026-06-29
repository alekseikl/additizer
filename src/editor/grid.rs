use egui::{
    Color32, Painter, Pos2, Rect, ScrollArea, Sense, Shape, Ui, Vec2,
    epaint::{CubicBezierShape, PathStroke},
    pos2,
    scroll_area::{ScrollBarVisibility, ScrollSource},
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
const WIRE_END_DOT: f32 = 8.0;

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

impl GridVec {
    fn from_vec_rounded(value: Vec2) -> Self {
        Self {
            x: (value.x / GRID_CELL_SIZE).round() as i32,
            y: (value.y / GRID_CELL_SIZE).round() as i32,
        }
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
    selected_module_id: Option<ModuleId>,
    opened_module_id: Option<ModuleId>,
}

pub struct Grid {
    widgets: Vec<GridWidget>,
    widgets_state: WidgetsState,
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

    pub fn ui(
        &mut self,
        ui: &mut Ui,
        bridge: &mut UiBridge,
        selected_module_id: Option<ModuleId>,
    ) -> Option<ModuleId> {
        let content_size = self.calc_content_size(bridge);
        let dragging = self.widgets.iter().any(GridWidget::is_dragging)
            || self.widgets_state.wire_drag.is_some();

        self.content_size = if dragging {
            self.content_size.max(content_size)
        } else {
            content_size
        };

        let viewport_size = ui.available_size();

        // Never smaller than the viewport so the grid fills the panel.
        let grid_area = (self.content_size + 0.5 * viewport_size).max(viewport_size);

        ScrollArea::both()
            .scroll_source(ScrollSource {
                drag: true,
                scroll_bar: false,
                ..Default::default()
            })
            .wheel_scroll_multiplier(TRACKPAD_SCROLL_MULTIPLIER)
            .scroll_bar_visibility(ScrollBarVisibility::AlwaysHidden)
            .auto_shrink([true, true])
            .show(ui, |ui| {
                let (response, painter) = ui.allocate_painter(grid_area, Sense::hover());

                Self::paint_grid(&painter, painter.clip_rect(), response.rect.min);

                // Reserve a paint slot for the wires.
                let wires = painter.add(Shape::Noop);

                let (opened_module_id, moved_module_id) = {
                    let mut ctx = WidgetCtx {
                        bridge,
                        state: &mut self.widgets_state,
                        moved_module_id: None,
                        selected_module_id,
                        opened_module_id: None,
                    };

                    for widget in &mut self.widgets {
                        widget.ui(ui, &mut ctx);
                    }

                    (ctx.opened_module_id, ctx.moved_module_id)
                };

                painter.set(wires, Shape::Vec(self.build_wire_shapes()));

                if let Some(drag) = self.widgets_state.wire_drag.as_mut()
                    && let Some(dropped_at) = drag.dropped_at
                    && dropped_at < ui.ctx().cumulative_frame_nr()
                {
                    self.widgets_state.wire_drag = None;
                }

                if let Some(drag) = &self.widgets_state.wire_drag
                    && let Some(pointer) = ui.ctx().pointer_hover_pos()
                {
                    painter.add(self.build_drag_wire_shape(drag, pointer));
                }

                if let Some(anchor) = moved_module_id {
                    self.resolve_overlaps(anchor, bridge);
                }

                opened_module_id
            })
            .inner
    }

    fn calc_content_size(&self, bridge: &UiBridge) -> Vec2 {
        let mut extent = Vec2::ZERO;

        for widget in &self.widgets {
            let pos = bridge.get_module_position(widget.module_id());
            let cell_extent = pos + widget.grid_size();
            let bottom_right = Vec2::from(cell_extent) + widget.drag_offset();

            extent = extent.max(bottom_right);
        }

        extent
    }

    fn build_drag_wire_shape(&self, drag: &WireDragState, pointer: Pos2) -> Shape {
        let src_pos = drag.start_pos;
        let dst_pos = pointer;
        let output_color = drag.color;
        let dx = ((dst_pos.x - src_pos.x).abs() * 0.5).max(WIRE_CTRL_MIN);
        let ctrl1 = src_pos + vec2(dx, 0.0);
        let ctrl2 = dst_pos - vec2(dx, 0.0);
        let stroke = PathStroke::new(WIRE_T, output_color).middle();

        Shape::Vec(vec![
            Shape::CubicBezier(CubicBezierShape::from_points_stroke(
                [src_pos, ctrl1, ctrl2, dst_pos],
                false,
                Color32::TRANSPARENT,
                stroke,
            )),
            Shape::circle_filled(dst_pos, WIRE_END_DOT * 0.5, output_color),
        ])
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

    fn build_wire_shapes(&self) -> Vec<Shape> {
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

    fn trim_partial_cell(span: f32) -> f32 {
        (span / GRID_CELL_SIZE).floor() * GRID_CELL_SIZE
    }

    fn paint_grid(painter: &Painter, area: Rect, origin: Pos2) {
        let stroke = PathStroke::new(GRID_T, C_GRID).inside();

        painter.rect_filled(area, 0.0, Color32::BLACK);

        let mut x = origin.x + Self::trim_partial_cell(area.left() - origin.x);

        while x <= area.right() {
            painter.line(
                vec![pos2(x, area.top()), pos2(x, area.bottom())],
                stroke.clone(),
            );
            x += GRID_CELL_SIZE;
        }

        let mut y = origin.y + Self::trim_partial_cell(area.top() - origin.y);

        while y <= area.bottom() {
            painter.line(
                vec![pos2(area.left(), y), pos2(area.right(), y)],
                stroke.clone(),
            );
            y += GRID_CELL_SIZE;
        }
    }
}
