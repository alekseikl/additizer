use egui::{Context, Id, Rect, Response, Sense, Ui, UiBuilder, pos2, vec2};

use addi_dsp::segments::time_segments::{TimePoint, TimeSegments};

use super::TimeSegmentsEditor;
use super::utils::Ctx;

fn plot() -> Rect {
    Rect::from_min_max(pos2(0.0, 0.0), pos2(100.0, 100.0))
}

fn ctx<'a>(ui: &'a Ui, response: &'a Response, segments: &'a mut TimeSegments) -> Ctx<'a> {
    Ctx {
        ui,
        plot: plot(),
        res: response,
        segs: segments,
        span: 4.0,
    }
}

fn ui() -> Ui {
    Ui::new(
        Context::default(),
        Id::unique("time-segments-test"),
        UiBuilder::new(),
    )
}

#[test]
fn typed_point_keeps_the_entered_value() {
    let mut segments = TimeSegments::default();

    assert!(segments.add_point(
        1,
        0.5,
        TimePoint {
            y: 0.5,
            ..TimePoint::default()
        }
    ));

    let mut ui = ui();
    let (_, response) = ui.allocate_exact_size(vec2(100.0, 100.0), Sense::hover());
    let editor = TimeSegmentsEditor::default();

    editor.place_point(&mut ctx(&ui, &response, &mut segments), 1, 0.3, 0.3);

    assert!((segments.time(1) - 0.3).abs() < 1e-6);
    assert!((segments.point(1).y - 0.3).abs() < 1e-6);

    editor.place_point(&mut ctx(&ui, &response, &mut segments), 0, 1.0, 0.4);

    assert_eq!(segments.time(0), 0.0);
    assert!((segments.point(0).y - 0.4).abs() < 1e-6);

    editor.place_point(&mut ctx(&ui, &response, &mut segments), 2, 9.0, 2.0);

    assert!((segments.time(2) - 9.0).abs() < 1e-4);
    assert!((segments.point(2).y - 1.0).abs() < 1e-6);
}
