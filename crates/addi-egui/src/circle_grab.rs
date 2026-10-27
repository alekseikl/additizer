use egui::{Pos2, Ui, pos2};

/// A circle press stays a click until the pointer moves this far.
const MAX_CLICK_DIST: f32 = 2.0;
/// A circle press stays a click until it has been held this long.
const MAX_CLICK_DURATION: f64 = 0.2;

/// Pointer grab of a circle. The center keeps the offset it had when the drag started.
#[derive(Clone, Copy, PartialEq)]
pub struct CircleGrab {
    pointer: Pos2,
    center: Pos2,
}

impl CircleGrab {
    pub fn new(pointer: Pos2, center: Pos2) -> Self {
        Self { pointer, center }
    }

    pub fn center(self) -> Pos2 {
        self.center
    }

    /// Circle center after the pointer moves to `pointer`.
    pub fn place(self, pointer: Pos2) -> Pos2 {
        pos2(
            self.center.x + pointer.x - self.pointer.x,
            self.center.y + pointer.y - self.pointer.y,
        )
    }

    /// Pointer once a press at `press_origin` is a drag.
    ///
    /// While it can still be a click, a repaint is requested so the hold can elapse.
    pub fn press(
        ui: &Ui,
        pointer: Option<Pos2>,
        press_origin: Pos2,
        press_start: Option<f64>,
        now: f64,
    ) -> Option<Pos2> {
        match press_ready(pointer, press_origin, press_start, now) {
            Ok(pos) => Some(pos),
            Err(after) => {
                ui.ctx().request_repaint_after_secs(after);
                None
            }
        }
    }
}

/// `Ok(pointer)` when the press is a drag, otherwise seconds left until the hold expires.
fn press_ready(
    pointer: Option<Pos2>,
    press_origin: Pos2,
    press_start: Option<f64>,
    now: f64,
) -> Result<Pos2, f32> {
    let moved = pointer.is_some_and(|pos| pos.distance(press_origin) > MAX_CLICK_DIST);
    let held = press_start.is_some_and(|start| now - start > MAX_CLICK_DURATION);

    if (moved || held)
        && let Some(pos) = pointer
    {
        return Ok(pos);
    }

    let remaining = press_start
        .map(|start| (MAX_CLICK_DURATION - (now - start)) as f32)
        .unwrap_or(MAX_CLICK_DURATION as f32);

    Err(remaining.max(0.0))
}

#[cfg(test)]
mod tests;
