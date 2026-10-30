use egui::{Pos2, Vec2, pos2};

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

#[cfg(test)]
mod tests;
