use egui::ecolor::Hsva;

use crate::{
    editor::{
        grid::WidgetCtx,
        waveform::{WaveformBuilder, WaveformOptions},
    },
    synth_engine::{
        ModuleId,
        spectral_mixer::{DISPLAY_SPECTRUM_SIZE, SpectralMixerUiBridge},
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

pub struct SpectralMixerWidget {
    waveform: WaveformBuilder,
}

impl Default for SpectralMixerWidget {
    fn default() -> Self {
        Self {
            waveform: WaveformBuilder::new(DISPLAY_SPECTRUM_SIZE),
        }
    }
}

impl SpectralMixerWidget {
    fn mixer_ui(&mut self, ui: &mut egui::Ui, mixer_bridge: &mut SpectralMixerUiBridge) {
        let size = ui.available_size();
        let response = ui.allocate_response(size, egui::Sense::hover());
        let rect = response.rect.shrink2(egui::vec2(0.0, WAVE_PADDING));
        let painter = ui.painter();

        if ui.is_rect_visible(rect) {
            self.waveform.build_and_paint(
                painter,
                rect,
                mixer_bridge.get_spectrum(),
                WaveformOptions {
                    color: WAVE_COLOR.into(),
                    ..Default::default()
                },
            );
        }
    }
}

impl GridWidgetContent for SpectralMixerWidget {
    fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut WidgetCtx, module_id: ModuleId) {
        ctx.bridge
            .with_module_bridge(module_id, |_bridge, module_bridge| {
                if let ModuleBridge::SpectralMixer(mixer_bridge) = module_bridge {
                    self.mixer_ui(ui, mixer_bridge);
                }
            });
    }
}
