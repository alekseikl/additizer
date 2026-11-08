use addi_dsp::segments::{
    CurveParam,
    time_segments::{MAX_TIME, TimePoint, TimeSegments},
};
use addi_dsp::utils::sanitize;
use addi_engine::Smp;
use egui::{
    Color32, LayerId, Painter, Pos2, Rect, Response, Sense, Stroke, Ui, Vec2, pos2, vec2,
};

use super::super::common;
use super::consts as C;

pub(super) use super::super::common::{Drag, Handle, with_alpha};

pub struct Ctx<'a> {
    pub ui: &'a Ui,
    pub plot: Rect,
    pub res: &'a Response,
    pub segs: &'a mut TimeSegments,
    pub span: Smp,
}

impl Ctx<'_> {
    pub fn shift_down(&self) -> bool {
        common::shift_down(self.ui)
    }

    pub fn handle_layer(&self) -> LayerId {
        common::handle_layer(self.ui, "time-segments-handle")
    }

    /// Clickable square around `center`, including the part outside the plot.
    /// A square that misses the plot has no target.
    pub fn handle_response(
        &self,
        center: Pos2,
        radius: f32,
        id: impl std::hash::Hash + std::fmt::Debug,
    ) -> Response {
        let grab = Rect::from_center_size(center, Vec2::splat(radius * 2.0));
        let rect = if grab.intersects(self.plot) {
            grab
        } else {
            Rect::NOTHING
        };

        self.ui
            .interact(rect, self.ui.make_persistent_id(id), Sense::click_and_drag())
    }

    pub fn add_point(&mut self, point: Pos2) {
        let index = self.insertion_index(point.x);
        let smooth = self.segs.point(index.saturating_sub(1)).curve.smooth;

        self.segs.add_point(
            index,
            point.x,
            TimePoint {
                y: point.y,
                curve: CurveParam {
                    smooth,
                    ..CurveParam::default()
                },
                ..TimePoint::default()
            },
        );
    }

    fn insertion_index(&self, time: Smp) -> usize {
        let mut at = 0.0;

        for index in 0..self.segs.num_points() {
            if at > time {
                return index;
            }

            at += self.segs.duration(index);
        }

        self.segs.num_points()
    }

    /// Absolute time a point may occupy. The start stays at 0. Moving any later point
    /// lengthens or shortens only the previous segment, until the curve reaches `MAX_TIME`.
    pub fn time_limits(&self, index: usize) -> (Smp, Smp) {
        let n = self.segs.num_points();

        if index == 0 || index >= n {
            return (0.0, 0.0);
        }

        let previous = self.segs.time(index - 1);
        let at = previous + self.segs.duration(index - 1);
        let tail = (self.segs.total_time() - at).max(0.0);
        let max = (MAX_TIME - tail).max(previous);

        (previous, max)
    }

    pub fn screen_pos(&self, time: Smp, level: Smp) -> Pos2 {
        pos2(self.screen_x(time), self.screen_y(level))
    }

    /// Plot x for `time`. Times past the window keep going, so points leave the right edge
    /// instead of stacking on it.
    pub fn screen_x(&self, time: Smp) -> f32 {
        let x = if self.span > 0.0 {
            time / self.span
        } else {
            0.0
        };

        self.plot.left() + x * self.plot.width()
    }

    pub fn screen_y(&self, level: Smp) -> f32 {
        self.plot.bottom() - level.clamp(0.0, 1.0) * self.plot.height()
    }

    /// `(seconds, level)` under `pos`. Time and level both continue past the plot, so a
    /// bend keeps following the pointer after it leaves the viewport.
    pub fn curve_pos(&self, pos: Pos2) -> Pos2 {
        self.curve_pos_at(pos, self.span)
    }

    /// Like [`curve_pos`](Self::curve_pos), mapping time through `span` seconds.
    pub fn curve_pos_at(&self, pos: Pos2, span: Smp) -> Pos2 {
        let width = self.plot.width().max(1.0);
        let height = self.plot.height().max(1.0);
        let time = sanitize((pos.x - self.plot.left()) / width * span, 0.0, MAX_TIME);
        let level = (self.plot.bottom() - pos.y) / height;
        let level = if level.is_finite() { level } else { 0.0 };

        pos2(time, level)
    }

    pub fn playhead_pos(&self, time: Smp, level: Smp) -> Pos2 {
        self.screen_pos(sanitize(time, 0.0, MAX_TIME), sanitize(level, 0.0, 1.0))
    }

    pub fn is_inner(&self, index: usize) -> bool {
        common::is_inner(self.segs.num_points(), index)
    }
}

/// Window while a point is dragged. It stays on `base` until the point's time passes the
/// right edge, then the edge follows that time. Releasing the point does not refit.
pub fn drag_span(base: Smp, time: Smp) -> Smp {
    base.max(time).clamp(C::MIN_WINDOW, MAX_TIME)
}

/// Positive `amount` shows fewer seconds. The window stays inside the ruler limits.
pub fn zoom_span(span: Smp, amount: f32) -> Smp {
    (span * 2f32.powf(-amount / C::ZOOM_POINTS)).clamp(C::MIN_WINDOW, MAX_TIME)
}

/// Window after a vertical grab of the zoom handle.
/// `delta_y` is the pointer movement in points, positive downward. Dragging up zooms in.
pub fn zoom_drag(span: Smp, delta_y: f32) -> Smp {
    zoom_span(span, -delta_y)
}

/// Lens center, and the icon bounds grown a little so the whole glass is easy to grab.
pub fn zoom_layout(plot: Rect) -> (Pos2, Rect) {
    let dir = vec2(1.0, 1.0).normalized();
    let lens = C::ZOOM_LENS * C::ZOOM_SCALE;
    let stroke = C::ZOOM_STROKE * C::ZOOM_SCALE;
    let outer = lens + stroke * 0.5;
    let tip = dir * (lens + C::ZOOM_HANDLE * C::ZOOM_SCALE + stroke * 0.5);
    let center = pos2(
        plot.right() - C::ZOOM_MARGIN - tip.x,
        plot.top() + C::ZOOM_MARGIN + outer,
    );
    let bounds = Rect::from_min_max(
        pos2(center.x - outer, center.y - outer),
        pos2(center.x + tip.x, center.y + tip.y),
    );

    (center, bounds.expand(2.0))
}

pub fn zoom_grab(plot: Rect) -> Rect {
    zoom_layout(plot).1
}

pub fn pointer_over_zoom(ui: &Ui, plot: Rect) -> bool {
    ui.input(|input| input.pointer.hover_pos())
        .is_some_and(|pos| zoom_grab(plot).contains(pos))
}

/// Thick ring and a round-capped handle toward the bottom-right.
pub fn paint_zoom_icon(painter: &Painter, center: Pos2, color: Color32) {
    let width = C::ZOOM_STROKE * C::ZOOM_SCALE;
    let lens = C::ZOOM_LENS * C::ZOOM_SCALE;
    let stroke = Stroke::new(width, color);
    let dir = vec2(1.0, 1.0).normalized();
    let cap = center + dir * (lens + C::ZOOM_HANDLE * C::ZOOM_SCALE);

    painter.circle_stroke(center, lens, stroke);
    painter.line_segment([center + dir * lens, cap], stroke);
    painter.circle_filled(cap, width * 0.5, color);
}

#[cfg(test)]
mod tests;
