use egui::ecolor::Hsva;

use crate::{
    editor::{
        grid::WidgetCtx,
        waveform::{WaveformBuilder, WaveformOptions},
    },
    synth_engine::{
        DISPLAY_SPECTRUM_SIZE, ModuleId,
        harmonic_editor::HarmonicEditorUiBridge,
        ui_bridge::ModuleBridge,
    },
};

use super::GridWidgetContent;

const WAVE_PADDING: f32 = 4.0;
const WAVE_COLOR: Hsva = Hsva {
    h: 0.567,
    s: 1.0,
    v: 0.5,
    a: 1.0,
};

pub struct HarmonicEditorWidget {
    waveform: WaveformBuilder,
}

impl Default for HarmonicEditorWidget {
    fn default() -> Self {
        Self {
            waveform: WaveformBuilder::new(DISPLAY_SPECTRUM_SIZE),
        }
    }
}

impl HarmonicEditorWidget {
    fn editor_ui(&mut self, ui: &mut egui::Ui, editor_bridge: &mut HarmonicEditorUiBridge) {
        let size = ui.available_size();
        let response = ui.allocate_response(size, egui::Sense::hover());
        let rect = response.rect.shrink2(egui::vec2(0.0, WAVE_PADDING));
        let painter = ui.painter();

        if ui.is_rect_visible(rect) {
            self.waveform.build_and_paint(
                painter,
                rect,
                editor_bridge.get_spectrum(),
                WaveformOptions {
                    color: WAVE_COLOR.into(),
                    ..Default::default()
                },
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
