use egui::{Pos2, pos2};

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
}

#[cfg(test)]
mod tests;
