use egui::pos2;

use super::CircleGrab;

#[test]
fn place_keeps_the_grab_offset() {
    let grab = CircleGrab::new(pos2(10.0, 10.0), pos2(14.0, 6.0));

    assert_eq!(grab.place(pos2(10.0, 10.0)), pos2(14.0, 6.0));
    assert_eq!(grab.place(pos2(12.0, 15.0)), pos2(16.0, 11.0));
}
