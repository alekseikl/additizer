use egui::{Color32, Painter, Pos2, Rect};

use crate::synth_engine::Sample;

const NUM_SEGMENTS: usize = 12;
const SEGMENT_GAP: f32 = 2.0;
const BAR_MAX_WIDTH: f32 = 24.0;
const BAR_MIN_WIDTH: f32 = 4.0;

const OFF_COLOR: Color32 = Color32::from_rgb(36, 38, 50);
const GREEN: Color32 = Color32::from_rgb(0x06, 0xaa, 0x1c);

#[derive(Default)]
pub struct ControlMeter {}

impl ControlMeter {
    pub fn paint_mono(&self, painter: &Painter, rect: Rect, value: Sample) {
        let bar_width = rect.width().clamp(BAR_MIN_WIDTH, BAR_MAX_WIDTH);
        let left = rect.center().x - bar_width * 0.5;
        let bar_rect = Rect::from_min_max(
            Pos2::new(left, rect.top()),
            Pos2::new(left + bar_width, rect.bottom()),
        );

        let segment_height =
            (bar_rect.height() - SEGMENT_GAP * (NUM_SEGMENTS - 1) as f32) / NUM_SEGMENTS as f32;

        if segment_height <= 0.0 {
            return;
        }

        for segment_idx in 0..NUM_SEGMENTS {
            let bottom = bar_rect.bottom() - segment_idx as f32 * (segment_height + SEGMENT_GAP);
            let segment_rect = Rect::from_min_max(
                Pos2::new(bar_rect.left(), bottom - segment_height),
                Pos2::new(bar_rect.right(), bottom),
            );
            let brightness = Self::segment_brightness(segment_idx, value);
            let color = if brightness <= 0.0 {
                OFF_COLOR
            } else {
                OFF_COLOR.lerp_to_gamma(GREEN, brightness)
            };

            painter.rect_filled(segment_rect, 0.0, color);
        }
    }

    fn segment_brightness(segment_idx: usize, value: Sample) -> Sample {
        let lower = segment_idx as Sample / NUM_SEGMENTS as Sample;
        let upper = (segment_idx + 1) as Sample / NUM_SEGMENTS as Sample;

        if value <= lower {
            0.0
        } else if value >= upper {
            1.0
        } else {
            (value - lower) / (upper - lower)
        }
    }
}
