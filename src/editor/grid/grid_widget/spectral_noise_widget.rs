use egui::ecolor::Hsva;

use crate::{
    editor::{
        grid::WidgetCtx,
        waveform::{WaveformBuilder, WaveformOptions},
    },
    synth_engine::{
        DISPLAY_SPECTRUM_SIZE, ModuleId, spectral_noise::SpectralNoiseUiBridge,
        ui_bridge::ModuleBridge,
    },
};

use super::GridWidgetContent;

const WAVE_PADDING: f32 = 4.0;
const WAVE_COLOR: Hsva = Hsva {
    h: 0.08,
    s: 0.85,
    v: 0.7,
    a: 1.0,
};

pub struct SpectralNoiseWidget {
    waveform: WaveformBuilder,
}

impl Default for SpectralNoiseWidget {
    fn default() -> Self {
        Self {
            waveform: WaveformBuilder::new(DISPLAY_SPECTRUM_SIZE),
        }
    }
}

impl SpectralNoiseWidget {
    fn noise_ui(&mut self, ui: &mut egui::Ui, noise_bridge: &mut SpectralNoiseUiBridge) {
        let size = ui.available_size();
        let response = ui.allocate_response(size, egui::Sense::hover());
        let rect = response.rect.shrink2(egui::vec2(0.0, WAVE_PADDING));
        let painter = ui.painter();

        if ui.is_rect_visible(rect) {
            self.waveform.build_and_paint(
                painter,
                rect,
                noise_bridge.get_display_spectrum(),
                WaveformOptions {
                    color: WAVE_COLOR.into(),
                    ..Default::default()
                },
            );
        }
    }
}

impl GridWidgetContent for SpectralNoiseWidget {
    fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut WidgetCtx, module_id: ModuleId) {
        ctx.bridge
            .with_module_bridge(module_id, |_bridge, module_bridge| {
                if let ModuleBridge::SpectralNoise(noise_bridge) = module_bridge {
                    self.noise_ui(ui, noise_bridge);
                }
            });
    }
}
