use egui::{Color32, Pos2, Rect, ecolor::Hsva};

use crate::{
    editor::grid::WidgetCtx,
    synth_engine::{
        ModuleId, SPECTRAL_BUFFER_SIZE, Sample, harmonic_editor::HarmonicEditorUiBridge,
        ui_bridge::ModuleBridge,
    },
};

use super::GridWidgetContent;

const PADDING: f32 = 4.0;
const BAR_WIDTH: f32 = 2.0;
const BAR_GAP: f32 = 1.0;
const BAR_STRIDE: f32 = BAR_WIDTH + BAR_GAP;

const MAX_DBS: Sample = 48.0;
const MID_POINT: Sample = 0.7;
const SKEW_FACTOR: Sample = 1.6;

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

const NUM_EDITABLE_HARMONICS: usize = SPECTRAL_BUFFER_SIZE - 1;

pub struct HarmonicEditorWidget {}

impl HarmonicEditorWidget {
    fn editor_ui(&mut self, ui: &mut egui::Ui, editor_bridge: &mut HarmonicEditorUiBridge) {
        let size = ui.available_size();
        let response = ui.allocate_response(size, egui::Sense::hover());
        let rect = response.rect.shrink2(egui::vec2(0.0, PADDING));

        if !rect.is_positive() || !ui.is_rect_visible(rect) {
            return;
        }

        let num_bars = (((rect.width() + BAR_GAP) / BAR_STRIDE).floor() as usize)
            .clamp(1, NUM_EDITABLE_HARMONICS);
        let harmonics = editor_bridge.harmonics();
        let painter = ui.painter().with_clip_rect(rect);

        for i in 0..num_bars {
            let gain = harmonics.get(i + 1).map(|h| h.left()).unwrap_or(0.0);
            let left = rect.left() + i as f32 * BAR_STRIDE;
            let bar_rect = Rect::from_min_max(
                Pos2::new(left, rect.top()),
                Pos2::new(left + BAR_WIDTH, rect.bottom()),
            );

            Self::paint_gain_bar(&painter, bar_rect, gain);
        }
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
        let norm = Self::gain_to_normalized(gain).clamp(0.0, 1.0);
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
}

impl GridWidgetContent for HarmonicEditorWidget {
    fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut WidgetCtx, module_id: ModuleId) {
        ctx.bridge
            .with_module_bridge(module_id, |_bridge, module_bridge| {
                if let ModuleBridge::HarmonicEditor(editor_bridge) = module_bridge {
                    self.editor_ui(ui, editor_bridge);
                }
            });
    }
}
