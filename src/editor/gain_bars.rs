use std::f32::consts::PI;

use egui::{Color32, Pos2, Rect, ecolor::Hsva};

use crate::synth_engine::{ComplexSample, Sample};

const PADDING: f32 = 4.0;
const BAR_WIDTH: f32 = 2.0;
const BAR_GAP: f32 = 1.0;
const BAR_STRIDE: f32 = BAR_WIDTH + BAR_GAP;

const MAX_DBS: Sample = 24.0;
const MID_POINT: Sample = 0.75;
const SKEW_FACTOR: Sample = 2.0;

const ATTENUATED_COLOR: Hsva = Hsva {
    h: 0.567,
    s: 1.0,
    v: 0.5,
    a: 1.0,
};
const AMPLIFIED_COLOR: Hsva = Hsva {
    h: 0.0,
    s: 0.95,
    v: 0.5,
    a: 1.0,
};

/// Recovers linear gain from a harmonic-series frequency bin (see
/// `HarmonicEditor::harmonics_from_config`).
fn bin_to_gain(harmonic_idx: usize, bin: ComplexSample) -> Sample {
    let value = harmonic_idx as Sample * PI * bin.norm();
    let almost_one = (value - 1.0).abs() < Sample::EPSILON;

    Sample::from(almost_one) * 1.0 + Sample::from(!almost_one) * value
}

fn gain_to_normalized(gain: Sample) -> Sample {
    let dbs = nih_plug::util::gain_to_db(gain);

    if dbs > 0.0 {
        let normalized = dbs / MAX_DBS;
        MID_POINT + (1.0 - MID_POINT) * normalized.powf(SKEW_FACTOR.recip())
    } else {
        let normalized = dbs / nih_plug::util::MINUS_INFINITY_DB;
        MID_POINT * (1.0 - normalized.powf(SKEW_FACTOR.recip()))
    }
}

fn paint_gain_bar(painter: &egui::Painter, rect: Rect, gain: Sample) {
    let norm = gain_to_normalized(gain).clamp(0.0, 1.0);
    let height = rect.height();

    if norm > MID_POINT {
        painter.rect_filled(
            Rect::from_min_max(
                Pos2::new(rect.left(), rect.top() + (1.0 - norm) * height),
                rect.max,
            ),
            0.0,
            Color32::from(AMPLIFIED_COLOR),
        );
        painter.rect_filled(
            Rect::from_min_max(
                Pos2::new(rect.left(), rect.top() + (1.0 - MID_POINT) * height),
                rect.max,
            ),
            0.0,
            Color32::from(ATTENUATED_COLOR),
        );
    } else if norm > 0.0 {
        painter.rect_filled(
            Rect::from_min_max(
                Pos2::new(rect.left(), rect.top() + (1.0 - norm) * height),
                rect.max,
            ),
            0.0,
            Color32::from(ATTENUATED_COLOR),
        );
    }
}

/// Paints vertical magnitude bars for `bins`, filling `ui`'s available space.
pub fn paint_gain_bars(ui: &mut egui::Ui, bins: &[ComplexSample]) {
    let size = ui.available_size();
    let response = ui.allocate_response(size, egui::Sense::hover());
    let rect = response.rect.shrink2(egui::vec2(0.0, PADDING));

    if !rect.is_positive() || !ui.is_rect_visible(rect) || bins.is_empty() {
        return;
    }

    let num_bars = (((rect.width() + BAR_GAP) / BAR_STRIDE).floor() as usize).clamp(1, bins.len());
    let painter = ui.painter().with_clip_rect(rect);

    for (i, &bin) in bins.iter().take(num_bars).enumerate() {
        let left = rect.left() + i as f32 * BAR_STRIDE;
        let bar_rect = Rect::from_min_max(
            Pos2::new(left, rect.top()),
            Pos2::new(left + BAR_WIDTH, rect.bottom()),
        );

        paint_gain_bar(&painter, bar_rect, bin_to_gain(i, bin));
    }
}
