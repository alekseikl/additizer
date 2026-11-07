use addi_dsp::segments::{
    CurveParam,
    time_segments::{MAX_TIME, TimePoint, TimeSegments},
};
use addi_dsp::utils::sanitize;
use addi_engine::Smp;
use egui::{LayerId, Pos2, Rect, Response, Ui, pos2};

use super::super::common::{self, MissedHandle};
use super::consts as C;

pub(super) use super::super::common::{Drag, Handle, with_alpha};

pub(super) struct Ctx<'a> {
    pub(super) ui: &'a Ui,
    pub(super) plot: Rect,
    pub(super) res: &'a Response,
    pub(super) segs: &'a mut TimeSegments,
    pub(super) span: Smp,
}

impl Ctx<'_> {
    pub(super) fn shift_down(&self) -> bool {
        common::shift_down(self.ui)
    }

    pub(super) fn handle_layer(&self) -> LayerId {
        common::handle_layer(self.ui, "time-segments-handle")
    }

    pub(super) fn handle_response(
        &self,
        center: Pos2,
        radius: f32,
        id: impl std::hash::Hash + std::fmt::Debug,
    ) -> Response {
        common::handle_response(self.ui, self.plot, center, radius, id, MissedHandle::Ignore)
    }

    pub(super) fn add_point(&mut self, point: Pos2) {
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
    pub(super) fn time_limits(&self, index: usize) -> (Smp, Smp) {
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

    pub(super) fn screen_pos(&self, time: Smp, level: Smp) -> Pos2 {
        pos2(self.screen_x(time), self.screen_y(level))
    }

    /// Plot x for `time`. Times past the window keep going, so points leave the right edge
    /// instead of stacking on it.
    pub(super) fn screen_x(&self, time: Smp) -> f32 {
        let x = if self.span > 0.0 {
            time / self.span
        } else {
            0.0
        };

        self.plot.left() + x * self.plot.width()
    }

    pub(super) fn screen_y(&self, level: Smp) -> f32 {
        self.plot.bottom() - level.clamp(0.0, 1.0) * self.plot.height()
    }

    /// `(seconds, level)` under `pos`. Time and level both continue past the plot, so a
    /// bend keeps following the pointer after it leaves the viewport.
    pub(super) fn curve_pos(&self, pos: Pos2) -> Pos2 {
        self.curve_pos_at(pos, self.span)
    }

    /// Like [`curve_pos`](Self::curve_pos), mapping time through `span` seconds.
    pub(super) fn curve_pos_at(&self, pos: Pos2, span: Smp) -> Pos2 {
        let width = self.plot.width().max(1.0);
        let height = self.plot.height().max(1.0);
        let time = sanitize((pos.x - self.plot.left()) / width * span, 0.0, MAX_TIME);
        let level = (self.plot.bottom() - pos.y) / height;
        let level = if level.is_finite() { level } else { 0.0 };

        pos2(time, level)
    }

    pub(super) fn playhead_pos(&self, time: Smp, level: Smp) -> Pos2 {
        self.screen_pos(sanitize(time, 0.0, MAX_TIME), sanitize(level, 0.0, 1.0))
    }

    pub(super) fn is_inner(&self, index: usize) -> bool {
        common::is_inner(self.segs.num_points(), index)
    }
}

/// Window while a point is dragged. It stays on `base` until the point's time passes the
/// right edge, then the edge follows that time. Releasing the point does not refit.
pub(super) fn drag_span(base: Smp, time: Smp) -> Smp {
    base.max(time).clamp(C::MIN_WINDOW, MAX_TIME)
}

/// Scroll up shows more seconds. The window stays inside the ruler limits.
pub(super) fn zoom_span(span: Smp, scroll_y: f32) -> Smp {
    (span * 2f32.powf(-scroll_y / C::ZOOM_POINTS)).clamp(C::MIN_WINDOW, MAX_TIME)
}

#[cfg(test)]
mod tests {
    use super::{drag_span, zoom_span};

    #[test]
    fn dragging_past_the_edge_grows_the_window() {
        assert!((drag_span(1.0, 0.4) - 1.0).abs() < 1e-6);
        assert!((drag_span(1.0, 1.6) - 1.6).abs() < 1e-6);
    }

    #[test]
    fn scroll_up_zooms_out() {
        let wider = zoom_span(4.0, -super::C::ZOOM_POINTS);

        assert!((wider - 8.0).abs() < 1e-4);
        assert!((zoom_span(0.2, 800.0) - 0.125).abs() < 1e-4);
        assert!(zoom_span(50.0, -800.0) <= 60.0);
    }
}
