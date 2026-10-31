use addi_dsp::{
    segments::{SegPoint, Segments},
    utils::sanitize,
};
use addi_engine::Smp;
use egui::{
    Color32, FontFamily, FontId, LayerId, Mesh, Order, Painter, Pos2, Rect, Response, Sense, Shape,
    Stroke, Ui, Vec2, epaint::Vertex, pos2,
};

use super::consts as C;

/// Handle held by the pointer.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Handle {
    Point(usize),
    Power(usize),
}

/// Pointer grab of a circle. `pointer` and `origin` are where the drag started.
/// Later centers keep that grab offset.
#[derive(Clone, Copy, PartialEq)]
pub(super) struct Drag {
    pointer: Pos2,
    origin: Pos2,
    center: Pos2,
    handle: Handle,
}

impl Drag {
    pub(super) fn new(pointer: Pos2, center: Pos2, handle: Handle) -> Self {
        Self {
            pointer,
            origin: center,
            center,
            handle,
        }
    }

    pub(super) fn handle(self) -> Handle {
        self.handle
    }

    /// Circle center after the pointer moves to `pointer`, keeping the grab offset from `origin`.
    pub(super) fn place(self, pointer: Pos2) -> Pos2 {
        pos2(
            self.origin.x + pointer.x - self.pointer.x,
            self.origin.y + pointer.y - self.pointer.y,
        )
    }

    /// Moves the center to follow `pointer`.
    /// Returns the new center and its offset from `origin`.
    pub(super) fn move_pos(&mut self, pointer: Pos2) -> (Pos2, Vec2) {
        let center = self.place(pointer);
        let delta = center - self.origin;

        self.center = center;
        (center, delta)
    }
}

/// Point editing, or one paint shape. Clicking the active shape returns to points.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Tool {
    Edit,
    Paint(Pattern),
}

pub(super) const TOOLS: [Tool; 6] = [
    Tool::Edit,
    Tool::Paint(Pattern::Step),
    Tool::Paint(Pattern::Half),
    Tool::Paint(Pattern::Up),
    Tool::Paint(Pattern::Down),
    Tool::Paint(Pattern::Tri),
];

/// Shape stamped into one grid column. High samples sit at the pointer; low samples sit on the bottom.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Pattern {
    Step,
    Half,
    Down,
    Up,
    Tri,
}

impl Tool {
    pub(super) fn tip(self) -> &'static str {
        match self {
            Self::Edit => "Edit points",
            Self::Paint(pattern) => pattern.tip(),
        }
    }
}

impl Pattern {
    /// `(along the column, height)`. Height `1` follows the pointer.
    pub(super) fn shape(self) -> &'static [(Smp, Smp)] {
        match self {
            Self::Step => &[(0.0, 1.0), (1.0, 1.0)],
            Self::Half => &[(0.0, 1.0), (0.5, 1.0), (0.5, 0.0), (1.0, 0.0)],
            Self::Down => &[(0.0, 1.0), (1.0, 0.0)],
            Self::Up => &[(0.0, 0.0), (1.0, 1.0)],
            Self::Tri => &[(0.0, 0.0), (0.5, 1.0), (1.0, 0.0)],
        }
    }

    pub(super) fn scaled(self, from_x: Smp, to_x: Smp, level: Smp) -> Vec<SegPoint> {
        self.shape()
            .iter()
            .map(|&(t, height)| SegPoint {
                x: from_x + t * (to_x - from_x),
                y: height * level,
                ..SegPoint::default()
            })
            .collect()
    }

    fn tip(self) -> &'static str {
        match self {
            Self::Step => "Paint a step",
            Self::Half => "Paint a half step",
            Self::Down => "Paint a falling ramp",
            Self::Up => "Paint a rising ramp",
            Self::Tri => "Paint a triangle",
        }
    }
}

/// Filled mouse-cursor arrow, centered in the icon and leaning left.
/// The outline is concave at the tail, so it is filled from explicit triangles.
pub(super) fn paint_edit_icon(painter: &Painter, icon: Rect, color: Color32) {
    let width = icon.height() * C::CURSOR_ICON_ASPECT;
    let left = icon.center().x - width * 0.5;
    let vertices = C::CURSOR_ICON.map(|(x, y)| Vertex {
        pos: pos2(left + x * width, icon.bottom() - y * icon.height()),
        color,
        uv: pos2(0.0, 0.0),
    });
    let mut mesh = Mesh::default();

    for &(a, b, c) in &C::CURSOR_TRIANGLES {
        let base = mesh.vertices.len() as u32;

        mesh.vertices.extend([a, b, c].map(|index| vertices[index]));
        mesh.add_triangle(base, base + 1, base + 2);
    }

    painter.add(Shape::mesh(mesh));
}

/// Thin outlined waveform of one pattern, without fill. Each path stays open
/// along the bottom, like the triangle.
pub(super) fn paint_pattern_icon(painter: &Painter, icon: Rect, pattern: Pattern, color: Color32) {
    let path: &[(Smp, Smp)] = match pattern {
        Pattern::Step => &[(0.0, 0.0), (0.0, 1.0), (1.0, 1.0), (1.0, 0.0)],
        Pattern::Half => &[(0.0, 0.0), (0.0, 1.0), (0.5, 1.0), (0.5, 0.0), (1.0, 0.0)],
        Pattern::Up => &[(0.0, 0.0), (1.0, 1.0), (1.0, 0.0)],
        Pattern::Down => &[(0.0, 0.0), (0.0, 1.0), (1.0, 0.0)],
        Pattern::Tri => &[(0.0, 0.0), (0.5, 1.0), (1.0, 0.0)],
    };
    let point = |(x, y): (Smp, Smp)| {
        pos2(
            icon.left() + x * icon.width(),
            icon.bottom() - y * icon.height(),
        )
    };

    painter.line(
        path.iter().map(|&p| point(p)).collect(),
        Stroke::new(C::TOOL_ICON_STROKE, color),
    );
}

/// Ui, plot, pointer, and curve for one paint of the editor.
pub(super) struct Ctx<'a> {
    pub(super) ui: &'a Ui,
    pub(super) plot: Rect,
    pub(super) res: &'a Response,
    pub(super) stamping: bool,
    pub(super) segs: &'a mut Segments,
}

impl Ctx<'_> {
    pub(super) fn shift_down(&self) -> bool {
        self.ui.input(|input| input.modifiers.shift)
    }

    pub(super) fn handle_layer(&self) -> LayerId {
        LayerId::new(Order::Foreground, self.ui.id().with("segments-handle"))
    }

    /// Clickable square around `center`, kept inside the plot.
    pub(super) fn handle_response(
        &self,
        center: Pos2,
        radius: f32,
        id: impl std::hash::Hash + std::fmt::Debug,
    ) -> Response {
        let rect = Rect::from_center_size(center, Vec2::splat(radius * 2.0)).intersect(self.plot);
        let rect = if rect.is_positive() {
            rect
        } else {
            Rect::from_center_size(center, Vec2::splat(1.0))
        };

        self.ui.interact(
            rect,
            self.ui.make_persistent_id(id),
            Sense::click_and_drag(),
        )
    }

    /// Insert a point at `point`, with the easing of the segment it splits.
    pub(super) fn add_point(&mut self, point: Pos2) {
        let index = self.insertion_index(point.x);
        let smooth = self.segs.point(index.saturating_sub(1)).smooth;

        self.segs.add_point(
            index,
            SegPoint {
                x: point.x,
                y: point.y,
                smooth,
                ..SegPoint::default()
            },
        );
    }

    /// First point whose phase is past `x`, or the end of the curve.
    fn insertion_index(&self, x: Smp) -> usize {
        (0..self.segs.num_points())
            .find(|&index| self.segs.point(index).x > x)
            .unwrap_or(self.segs.num_points())
    }

    /// Phase range a point may occupy. Endpoints stay pinned to 0 and 1.
    pub(super) fn x_limits(&self, index: usize) -> (Smp, Smp) {
        let n = self.segs.num_points();

        if index == 0 {
            (0.0, 0.0)
        } else if index + 1 == n {
            (1.0, 1.0)
        } else {
            (self.segs.point(index - 1).x, self.segs.point(index + 1).x)
        }
    }

    /// Screen position of the playhead at `(phase, level)`.
    pub(super) fn playhead_pos(&self, phase: Smp, level: Smp) -> Pos2 {
        let phase = sanitize(phase, 0.0, 1.0);
        let level = sanitize(level, 0.0, 1.0);

        self.screen_pos(pos2(phase, level))
    }

    /// Plot position of a screen point. Values outside 0–1 are past the canvas.
    pub(super) fn curve_pos(&self, pos: Pos2) -> Pos2 {
        pos2(
            (pos.x - self.plot.left()) / self.plot.width(),
            (self.plot.bottom() - pos.y) / self.plot.height(),
        )
    }

    /// Screen position of a plot point. Values outside 0–1 land on the plot edge.
    pub(super) fn screen_pos(&self, point: Pos2) -> Pos2 {
        pos2(
            self.screen_x(point.x),
            self.plot.bottom() - point.y.clamp(0.0, 1.0) * self.plot.height(),
        )
    }

    pub(super) fn screen_x(&self, x: Smp) -> f32 {
        self.plot.left() + x.clamp(0.0, 1.0) * self.plot.width()
    }

    /// Interior points can be removed. The ends stay.
    pub(super) fn is_inner(&self, index: usize) -> bool {
        index > 0 && index + 1 < self.segs.num_points()
    }
}

/// Curve-space position of a breakpoint.
pub(super) fn curve_point(point: SegPoint) -> Pos2 {
    pos2(point.x, point.y)
}

/// Top of a box `height` tall placed `offset` above `anchor_y`, or `offset` below it
/// when the box would pass `top`.
pub(super) fn stack_y(anchor_y: f32, offset: f32, height: f32, top: f32) -> f32 {
    let above = anchor_y - offset - height;

    if above >= top {
        above
    } else {
        anchor_y + offset
    }
}

pub(super) fn bold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("Bold".into()))
}

pub(super) fn with_alpha(color: Color32, alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha)
}

#[cfg(test)]
mod tests;
