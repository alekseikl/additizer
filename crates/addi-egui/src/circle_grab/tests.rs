use egui::pos2;

use super::{CircleGrab, press_ready};

#[test]
fn place_keeps_the_grab_offset() {
    let grab = CircleGrab::new(pos2(10.0, 10.0), pos2(14.0, 6.0));

    assert_eq!(grab.place(pos2(10.0, 10.0)), pos2(14.0, 6.0));
    assert_eq!(grab.place(pos2(12.0, 15.0)), pos2(16.0, 11.0));
}

#[test]
fn press_waits_until_the_pointer_moves_or_the_hold_elapses() {
    let origin = pos2(0.0, 0.0);
    let pointer = Some(pos2(1.0, 0.0));

    assert!(press_ready(pointer, origin, Some(0.0), 0.0).is_err());
    assert_eq!(
        press_ready(Some(pos2(3.0, 0.0)), origin, Some(0.0), 0.0),
        Ok(pos2(3.0, 0.0))
    );
    assert_eq!(
        press_ready(pointer, origin, Some(0.0), 0.3),
        Ok(pos2(1.0, 0.0))
    );
}
