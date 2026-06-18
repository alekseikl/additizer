use egui::{
    Align2, Color32, FontId, Painter, Pos2, Rect, ScrollArea, Sense, Stroke, StrokeKind, Ui,
    Vec2, scroll_area::ScrollSource, vec2,
};

use crate::synth_engine::{ModuleId, OUTPUT_MODULE_ID, ui_bridge::UiBridge};

// ─── layout constants ────────────────────────────────────────────────────────

/// One grid cell in pixels.
const CELL: f32 = 80.0;
/// Module widget dimensions: 2 cells wide, 1 cell tall.
const MOD_W: f32 = CELL * 2.0;
const MOD_H: f32 = CELL;
/// Padding between the canvas edge and the grid origin, so outlets are never clipped.
const CANVAS_PAD: f32 = OUTLET_R + 4.0;
/// Virtual canvas extent — the ScrollArea scrolls over this area.
const VIRTUAL_W: f32 = 4000.0;
const VIRTUAL_H: f32 = 3000.0;

// ─── visual constants ────────────────────────────────────────────────────────

const C_GRID: Color32 = Color32::from_rgb(52, 52, 52);
const C_MOD_BG: Color32 = Color32::from_rgb(28, 30, 42);
const C_MOD_BG_DRAG: Color32 = Color32::from_rgb(46, 48, 66);
const C_MOD_BORDER: Color32 = Color32::from_rgb(76, 80, 118);
const C_LABEL: Color32 = Color32::from_rgb(188, 194, 224);
const C_SEP: Color32 = Color32::from_rgb(54, 57, 82);
const C_OUTLET: Color32 = Color32::from_rgb(178, 196, 242);
const C_WIRE: Color32 = Color32::from_rgb(138, 106, 228);

const OUTLET_R: f32 = 5.5;
const OUTLET_T: f32 = 1.5;
const WIRE_T: f32 = 1.5;
const GRID_T: f32 = 0.5;
const DASH: f32 = 4.0;
const GAP: f32 = 4.0;
/// Height of the label bar at the top of each module widget.
const LABEL_H: f32 = 22.0;

// ─── RoutingGrid ─────────────────────────────────────────────────────────────

/// Dragging state: which module is being dragged and how far from its snapped position.
type DragState = Option<(ModuleId, Vec2)>;

/// A canvas-style routing view. Displays all synth modules as draggable widgets
/// connected by Bézier wires according to the current `UiBridge` routing state.
///
/// Place in `EditorState` and call `show` every frame.
#[derive(Default)]
pub struct RoutingGrid {
    drag: DragState,
}

impl RoutingGrid {
    pub fn show(&mut self, bridge: &mut UiBridge, ui: &mut Ui) {
        ScrollArea::both()
            .scroll_source(ScrollSource { drag: false, ..Default::default() })
            .auto_shrink([false, false])
            .show(ui, |ui| {
                self.show_canvas(bridge, ui);
            });
    }

    fn show_canvas(&mut self, bridge: &mut UiBridge, ui: &mut Ui) {
        let (response, painter) =
            ui.allocate_painter(vec2(VIRTUAL_W, VIRTUAL_H), Sense::drag());

        let canvas = response.rect;
        let origin = canvas.min + vec2(CANVAS_PAD, CANVAS_PAD);

        // Background fills the full virtual canvas.
        painter.rect_filled(canvas, 0.0, Color32::BLACK);

        // Grid lines — only draw within the visible clip rect for efficiency,
        // but aligned to the canvas origin so they scroll with the content.
        paint_grid(&painter, painter.clip_rect(), origin);

        // ── gather data ──────────────────────────────────────────────────────

        let modules = bridge.get_modules();
        let links = bridge.get_all_links();

        // Build the screen rect for every module, applying any in-flight drag offset.
        let rects: Vec<(ModuleId, String, Rect)> = modules
            .iter()
            .map(|m| {
                let (gx, gy) = bridge.get_module_position(m.id);
                let drag_off = drag_offset_for(&self.drag, m.id);
                let tl = origin + vec2(gx as f32 * CELL, gy as f32 * CELL) + drag_off;
                (m.id, m.label.clone(), Rect::from_min_size(tl, vec2(MOD_W, MOD_H)))
            })
            .collect();

        // ── drag interaction ─────────────────────────────────────────────────

        if response.drag_started() {
            if let Some(ptr) = response.interact_pointer_pos() {
                for (id, _, rect) in &rects {
                    if rect.contains(ptr) {
                        self.drag = Some((*id, Vec2::ZERO));
                        break;
                    }
                }
            }
        }

        if response.dragged() {
            if let Some((_, off)) = &mut self.drag {
                *off += response.drag_delta();
            }
        }

        if response.drag_stopped() {
            if let Some((id, off)) = self.drag.take() {
                let (gx, gy) = bridge.get_module_position(id);
                let raw = vec2(gx as f32 * CELL + off.x, gy as f32 * CELL + off.y);
                let nx = (raw.x / CELL).round() as i32;
                let ny = (raw.y / CELL).round() as i32;
                bridge.set_module_position(id, nx.max(0), ny.max(0));
            }
        }

        // ── paint wires (below modules) ──────────────────────────────────────

        let find_rect = |id: ModuleId| rects.iter().find(|(mid, _, _)| *mid == id).map(|(_, _, r)| *r);

        for (src_id, dst_id) in &links {
            if let (Some(src_r), Some(dst_r)) = (find_rect(*src_id), find_rect(*dst_id)) {
                let out_pt = outlet_pos(src_r, true);
                let in_pt = outlet_pos(dst_r, false);
                paint_wire(&painter, out_pt, in_pt);
            }
        }

        // ── paint modules (on top of wires) ──────────────────────────────────

        for (id, label, rect) in &rects {
            let dragging = self.drag.is_some_and(|(did, _)| did == *id);
            let has_output = *id != OUTPUT_MODULE_ID;
            paint_module(&painter, *rect, label, dragging, has_output);
        }
    }
}

// ─── helpers ─────────────────────────────────────────────────────────────────

fn drag_offset_for(drag: &DragState, id: ModuleId) -> Vec2 {
    match drag {
        Some((did, off)) if *did == id => *off,
        _ => Vec2::ZERO,
    }
}

/// Y coordinate of the outlet circle, centred in the body area below the label.
fn outlet_y(rect: Rect) -> f32 {
    rect.min.y + LABEL_H + (rect.max.y - rect.min.y - LABEL_H) * 0.5
}

/// Screen position of a module's output (`right=true`) or input (`right=false`) outlet.
fn outlet_pos(rect: Rect, right: bool) -> Pos2 {
    let y = outlet_y(rect);
    if right {
        Pos2::new(rect.max.x, y)
    } else {
        Pos2::new(rect.min.x, y)
    }
}

// ─── painters ────────────────────────────────────────────────────────────────

fn paint_grid(painter: &Painter, area: Rect, origin: Pos2) {
    let stroke = Stroke::new(GRID_T, C_GRID);

    // Vertical dashed lines — start at the first column that is ≤ area.left()
    // and is aligned to the canvas origin, so lines scroll with the content.
    let x0 = origin.x + ((area.left() - origin.x) / CELL).floor() * CELL;
    let mut x = x0;
    while x <= area.right() {
        let mut y = area.top();
        while y < area.bottom() {
            let y2 = (y + DASH).min(area.bottom());
            painter.line_segment([Pos2::new(x, y), Pos2::new(x, y2)], stroke);
            y += DASH + GAP;
        }
        x += CELL;
    }

    // Horizontal dashed lines — same origin-relative alignment.
    let y0 = origin.y + ((area.top() - origin.y) / CELL).floor() * CELL;
    let mut y = y0;
    while y <= area.bottom() {
        let mut x = area.left();
        while x < area.right() {
            let x2 = (x + DASH).min(area.right());
            painter.line_segment([Pos2::new(x, y), Pos2::new(x2, y)], stroke);
            x += DASH + GAP;
        }
        y += CELL;
    }
}

fn paint_module(painter: &Painter, rect: Rect, label: &str, dragging: bool, has_output: bool) {
    let bg = if dragging { C_MOD_BG_DRAG } else { C_MOD_BG };
    painter.rect_filled(rect, 5.0, bg);
    painter.rect_stroke(rect, 5.0, Stroke::new(1.0, C_MOD_BORDER), StrokeKind::Inside);

    // Separator between label area and body.
    let sep_y = rect.min.y + LABEL_H;
    painter.line_segment(
        [Pos2::new(rect.min.x + 6.0, sep_y), Pos2::new(rect.max.x - 6.0, sep_y)],
        Stroke::new(0.5, C_SEP),
    );

    // Module label centred in the label area.
    painter.text(
        Pos2::new(rect.center().x, rect.min.y + LABEL_H * 0.5),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(11.0),
        C_LABEL,
    );

    let oy = outlet_y(rect);

    // Input outlet — hollow circle on the left edge.
    painter.circle_stroke(
        Pos2::new(rect.min.x, oy),
        OUTLET_R,
        Stroke::new(OUTLET_T, C_OUTLET),
    );

    // Output outlet — hollow circle on the right edge (omitted for the Output module).
    if has_output {
        painter.circle_stroke(
            Pos2::new(rect.max.x, oy),
            OUTLET_R,
            Stroke::new(OUTLET_T, C_OUTLET),
        );
    }
}

fn paint_wire(painter: &Painter, from: Pos2, to: Pos2) {
    // Cubic Bézier with horizontal tangents; control-point distance scales with
    // the horizontal span so the curve stays smooth even for short connections.
    let ctrl_dx = ((to.x - from.x).abs() * 0.5).max(CELL);
    let p1 = Pos2::new(from.x + ctrl_dx, from.y);
    let p2 = Pos2::new(to.x - ctrl_dx, to.y);

    const STEPS: usize = 32;
    let pts: Vec<Pos2> = (0..=STEPS)
        .map(|i| {
            let t = i as f32 / STEPS as f32;
            let u = 1.0 - t;
            Pos2::new(
                u * u * u * from.x
                    + 3.0 * u * u * t * p1.x
                    + 3.0 * u * t * t * p2.x
                    + t * t * t * to.x,
                u * u * u * from.y
                    + 3.0 * u * u * t * p1.y
                    + 3.0 * u * t * t * p2.y
                    + t * t * t * to.y,
            )
        })
        .collect();

    painter.line(pts, Stroke::new(WIRE_T, C_WIRE));
}
