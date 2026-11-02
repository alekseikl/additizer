use addi_dsp::time_segments::{MAX_TIME, TimePoint, TimeSegments};
use addi_dsp::utils::sanitize;
use addi_engine::Smp;
use egui::{
    Color32, FontFamily, FontId, LayerId, Order, Pos2, Rect, Response, Sense, Ui, Vec2, pos2,
};

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

    pub(super) fn add_point(&mut self, point: Pos2) {
        let index = self.insertion_index(point.x);
        let smooth = self.segs.point(index.saturating_sub(1)).smooth;

        self.segs.add_point(
            index,
            TimePoint {
                time: point.x,
                y: point.y,
                smooth,
                ..TimePoint::default()
            },
        );
    }

    fn insertion_index(&self, time: Smp) -> usize {
        (0..self.segs.num_points())
            .find(|&index| self.segs.point(index).time > time)
            .unwrap_or(self.segs.num_points())
    }

    /// Time range a point may occupy. The start stays at 0. The end can grow.
    pub(super) fn time_limits(&self, index: usize) -> (Smp, Smp) {
        let n = self.segs.num_points();

        if index == 0 {
            (0.0, 0.0)
        } else if index + 1 == n {
            (self.segs.point(index - 1).time, MAX_TIME)
        } else {
            (
                self.segs.point(index - 1).time,
                self.segs.point(index + 1).time,
            )
        }
    }

    pub(super) fn screen_pos(&self, time: Smp, level: Smp) -> Pos2 {
        pos2(self.screen_x(time), self.screen_y(level))
    }

    pub(super) fn screen_x(&self, time: Smp) -> f32 {
        let x = if self.span > 0.0 {
            time / self.span
        } else {
            0.0
        };

        self.plot.left() + x.clamp(0.0, 1.0) * self.plot.width()
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
        self.screen_pos(
            sanitize(time, 0.0, self.span.max(0.0)),
            sanitize(level, 0.0, 1.0),
        )
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
