use addi_dsp::time_segments::{MAX_TIME, TimePoint, TimeSegments};
use addi_dsp::utils::sanitize;
use addi_engine::Smp;
use egui::{
    Color32, FontFamily, FontId, LayerId, Order, Pos2, Rect, Response, Sense, Ui, Vec2, pos2,
};

use super::consts as C;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Handle {
    Point(usize),
    Power(usize),
}

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

    pub(super) fn place(self, pointer: Pos2) -> Pos2 {
        pos2(
            self.origin.x + pointer.x - self.pointer.x,
            self.origin.y + pointer.y - self.pointer.y,
        )
    }

    pub(super) fn move_pos(&mut self, pointer: Pos2) -> (Pos2, Vec2) {
        let center = self.place(pointer);
        let delta = center - self.origin;

        self.center = center;
        (center, delta)
    }
}

pub(super) struct Ctx<'a> {
    pub(super) ui: &'a Ui,
    pub(super) plot: Rect,
    pub(super) res: &'a Response,
    pub(super) segs: &'a mut TimeSegments,
    pub(super) span: Smp,
}

impl Ctx<'_> {
    pub(super) fn shift_down(&self) -> bool {
        self.ui.input(|input| input.modifiers.shift)
    }

    pub(super) fn handle_layer(&self) -> LayerId {
        LayerId::new(Order::Foreground, self.ui.id().with("time-segments-handle"))
    }

    pub(super) fn handle_response(
        &self,
        center: Pos2,
        radius: f32,
        id: impl std::hash::Hash + std::fmt::Debug,
    ) -> Response {
        let grab = Rect::from_center_size(center, Vec2::splat(radius * 2.0));
        let rect = grab.intersect(self.plot);
        let rect = if rect.is_positive() {
            rect
        } else if self.plot.contains(center) {
            Rect::from_center_size(center, Vec2::splat(1.0))
        } else {
            Rect::NOTHING
        };

        self.ui.interact(
            rect,
            self.ui.make_persistent_id(id),
            Sense::click_and_drag(),
        )
    }

    pub(super) fn add_point(&mut self, point: Pos2) {
        let index = self.insertion_index(point.x);
        let smooth = self.segs.point(index.saturating_sub(1)).smooth;

        self.segs.add_point(
            index,
            point.x,
            TimePoint {
                y: point.y,
                smooth,
                ..TimePoint::default()
            },
        );
    }

    fn insertion_index(&self, time: Smp) -> usize {
        (0..self.segs.num_points())
            .find(|&index| self.segs.time(index) > time)
            .unwrap_or(self.segs.num_points())
    }

    /// Absolute time a point may occupy. The start stays at 0. Moving any later point
    /// lengthens or shortens only the previous segment, until the curve reaches `MAX_TIME`.
    pub(super) fn time_limits(&self, index: usize) -> (Smp, Smp) {
        let n = self.segs.num_points();

        if index == 0 || index >= n {
            return (0.0, 0.0);
        }

        let previous = self.segs.time(index - 1);
        let tail = (self.segs.total_time() - self.segs.time(index)).max(0.0);
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

    /// `(seconds, level)` under `pos`. Time may run past the view when the pointer does.
    pub(super) fn curve_pos(&self, pos: Pos2) -> Pos2 {
        let width = self.plot.width().max(1.0);
        let height = self.plot.height().max(1.0);
        let time = sanitize(
            (pos.x - self.plot.left()) / width * self.span,
            0.0,
            MAX_TIME,
        );
        let level = sanitize((self.plot.bottom() - pos.y) / height, 0.0, 1.0);

        pos2(time, level)
    }

    pub(super) fn playhead_pos(&self, time: Smp, level: Smp) -> Pos2 {
        self.screen_pos(sanitize(time, 0.0, MAX_TIME), sanitize(level, 0.0, 1.0))
    }

    pub(super) fn is_inner(&self, index: usize) -> bool {
        index > 0 && index + 1 < self.segs.num_points()
    }
}

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

/// Seconds between ruler lines for a window of `span` seconds.
///
/// Major lines are `big` apart and minor lines are `little` apart. `minor` is how
/// strongly to draw the minor lines: 1 on a clean zoom step, fading to 0 before the
/// next step. The same spacing Vital uses for its envelope ruler.
pub(super) struct TimeDivisions {
    pub big: Smp,
    pub little: Smp,
    pub minor: f32,
}

pub(super) fn time_divisions(span: Smp) -> Option<TimeDivisions> {
    if span <= 0.0 {
        return None;
    }

    let powers = span.log(C::RULER_DIVISION);

    if !powers.is_finite() {
        return None;
    }

    let division = powers.floor();
    let transition = powers - division;
    let big = C::RULER_DIVISION.powf(division) / 2.0;
    let little = big / C::RULER_DIVISION;

    if little <= 0.0 || !little.is_finite() {
        return None;
    }

    Some(TimeDivisions {
        big,
        little,
        minor: (1.0 - transition).powi(2),
    })
}

/// Scroll up and pinch-out show fewer seconds. The window stays inside the ruler limits.
pub(super) fn zoom_span(span: Smp, scroll_y: f32, pinch: f32) -> Smp {
    let mut span = span * 2f32.powf(scroll_y / C::ZOOM_POINTS);

    if pinch > 0.0 {
        span /= pinch;
    }

    span.clamp(C::MIN_WINDOW, MAX_TIME)
}

#[cfg(test)]
mod tests {
    use super::{time_divisions, zoom_span};

    #[test]
    fn ruler_steps_follow_the_window() {
        let one = time_divisions(1.0).unwrap();

        assert!((one.big - 0.5).abs() < 1e-4);
        assert!((one.little - 0.125).abs() < 1e-4);
        assert!((one.minor - 1.0).abs() < 1e-4);

        let four = time_divisions(4.0).unwrap();

        assert!((four.big - 2.0).abs() < 1e-4);
        assert!((four.little - 0.5).abs() < 1e-4);
        assert!((four.minor - 1.0).abs() < 1e-4);

        let mid = time_divisions(2.0).unwrap();

        assert!((mid.big - 0.5).abs() < 1e-4);
        assert!((mid.little - 0.125).abs() < 1e-4);
        assert!((mid.minor - 0.25).abs() < 1e-4);
    }

    #[test]
    fn scroll_up_zooms_in() {
        let closer = zoom_span(4.0, -80.0, 1.0);

        assert!((closer - 2.0).abs() < 1e-4);
        assert!((zoom_span(4.0, 0.0, 2.0) - 2.0).abs() < 1e-4);
        assert!((zoom_span(0.2, -800.0, 1.0) - 0.125).abs() < 1e-4);
        assert!(zoom_span(50.0, 800.0, 1.0) <= 60.0);
    }
}
