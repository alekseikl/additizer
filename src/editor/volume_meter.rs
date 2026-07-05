use egui::{Color32, Painter, Pos2, Rect};

use crate::synth_engine::{Sample, StereoSample};

const BAR_GAP: f32 = 6.0;
const NUM_SEGMENTS: usize = 12;
const LINEAR_SEGMENTS: usize = 10;
const GREEN_SEGMENTS: usize = 8;
const SEGMENT_GAP: f32 = 2.0;
const MIN_DB: Sample = -48.0;
const LINEAR_MAX_DB: Sample = 0.0;

const OFF_COLOR: Color32 = Color32::from_rgb(36, 38, 50);
const GREEN: Color32 = Color32::from_rgb(0x06, 0xaa, 0x1c);
const YELLOW: Color32 = Color32::from_rgb(0xff, 0xcc, 0x00);
const RED: Color32 = Color32::from_rgb(0xe0, 0x30, 0x30);

pub fn paint_stereo(painter: &Painter, rect: Rect, volume: StereoSample) {
    let bar_width = ((rect.width() - BAR_GAP) * 0.5).clamp(4.0, 24.0);
    let total_width = bar_width * 2.0 + BAR_GAP;
    let left = rect.center().x - total_width * 0.5;

    let left_rect = Rect::from_min_max(
        Pos2::new(left, rect.top()),
        Pos2::new(left + bar_width, rect.bottom()),
    );
    let right_rect = Rect::from_min_max(
        Pos2::new(left + bar_width + BAR_GAP, rect.top()),
        Pos2::new(left + total_width, rect.bottom()),
    );

    paint_bar(painter, left_rect, volume.left());
    paint_bar(painter, right_rect, volume.right());
}

fn segment_color(segment_idx: usize) -> Color32 {
    if segment_idx < GREEN_SEGMENTS {
        GREEN
    } else if segment_idx < LINEAR_SEGMENTS {
        YELLOW
    } else {
        RED
    }
}

fn linear_segment_threshold(segment_idx: usize) -> Sample {
    MIN_DB + (segment_idx + 1) as Sample * (LINEAR_MAX_DB - MIN_DB) / LINEAR_SEGMENTS as Sample
}

fn segment_bounds(segment_idx: usize) -> (Sample, Sample) {
    match segment_idx {
        0 => (MIN_DB, linear_segment_threshold(0)),
        1..10 => (
            linear_segment_threshold(segment_idx - 1),
            linear_segment_threshold(segment_idx),
        ),
        10 => (3.0, 6.0),
        _ => (6.0, 12.0),
    }
}

fn segment_brightness(segment_idx: usize, db: Sample) -> Sample {
    let (lower, upper) = segment_bounds(segment_idx);

    if db <= lower {
        0.0
    } else if db >= upper {
        1.0
    } else {
        (db - lower) / (upper - lower)
    }
}

fn segment_fill_color(segment_idx: usize, brightness: Sample) -> Color32 {
    if brightness <= 0.0 {
        return OFF_COLOR;
    }

    OFF_COLOR.lerp_to_gamma(segment_color(segment_idx), brightness)
}

fn level_to_db(level: Sample) -> Sample {
    if level <= 1e-6 {
        return MIN_DB;
    }

    nih_plug::util::gain_to_db(level).max(MIN_DB)
}

fn paint_bar(painter: &Painter, rect: Rect, level: Sample) {
    let db = level_to_db(level);
    let segment_height =
        (rect.height() - SEGMENT_GAP * (NUM_SEGMENTS - 1) as f32) / NUM_SEGMENTS as f32;

    if segment_height <= 0.0 {
        return;
    }

    for segment_idx in 0..NUM_SEGMENTS {
        let bottom = rect.bottom() - segment_idx as f32 * (segment_height + SEGMENT_GAP);
        let segment_rect = Rect::from_min_max(
            Pos2::new(rect.left(), bottom - segment_height),
            Pos2::new(rect.right(), bottom),
        );
        let brightness = segment_brightness(segment_idx, db);
        let color = segment_fill_color(segment_idx, brightness);

        painter.rect_filled(segment_rect, 0.0, color);
    }
}
