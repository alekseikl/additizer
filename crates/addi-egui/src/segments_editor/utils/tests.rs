use egui::{pos2, vec2};

use super::{Drag, Handle};

#[test]
fn place_keeps_the_grab_offset() {
    let grab = Drag::new(pos2(10.0, 10.0), pos2(14.0, 6.0), Handle::Point(0));

    assert_eq!(grab.place(pos2(10.0, 10.0)), pos2(14.0, 6.0));
    assert_eq!(grab.place(pos2(12.0, 15.0)), pos2(16.0, 11.0));
}

#[test]
fn move_pos_advances_the_center() {
    let mut drag = Drag::new(pos2(10.0, 10.0), pos2(14.0, 6.0), Handle::Point(0));

    assert_eq!(
        drag.move_pos(pos2(12.0, 15.0)),
        (pos2(16.0, 11.0), vec2(2.0, 5.0))
    );
    assert_eq!(drag.center(), pos2(16.0, 11.0));
    assert_eq!(
        drag.move_pos(pos2(13.0, 15.0)),
        (pos2(17.0, 11.0), vec2(3.0, 5.0))
    );
    assert_eq!(
        drag.move_pos(pos2(10.0, 10.0)),
        (pos2(14.0, 6.0), vec2(0.0, 0.0))
    );
}
