use crate::{
    editor::{grid::WidgetCtx, waveform::WaveformBuilder},
    synth_engine::{
        ModuleId,
        oscillator::{DISPLAY_SPECTRUM_SIZE, OscillatorUiBridge},
        ui_bridge::{ModuleBridge, UiBridge},
    },
};

use super::GridWidgetContent;

const WAVE_PADDING: f32 = 4.0;

pub struct OscillatorWidget {
    waveform: WaveformBuilder,
}

impl Default for OscillatorWidget {
    fn default() -> Self {
        Self {
            waveform: WaveformBuilder::new(DISPLAY_SPECTRUM_SIZE),
        }
    }
}

impl OscillatorWidget {
    fn osc_ui(
        &mut self,
        ui: &mut egui::Ui,
        _bridge: &mut UiBridge,
        osc_bridge: &mut OscillatorUiBridge,
    ) {
        let size = ui.available_size();
        let response = ui.allocate_response(size, egui::Sense::hover());
        let rect = response.rect.shrink2(egui::vec2(0.0, WAVE_PADDING));
        let painter = ui.painter();

        if ui.is_rect_visible(rect) {
            self.waveform.build_and_paint(
                painter,
                rect,
                osc_bridge.get_spectrum(),
                Default::default(),
            );
        }
    }
}

impl GridWidgetContent for OscillatorWidget {
    fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut WidgetCtx, module_id: ModuleId) {
        ctx.bridge
            .with_module_bridge(module_id, |bridge, osc_bridge| {
                if let ModuleBridge::Oscillator(osc_bridge) = osc_bridge {
                    self.osc_ui(ui, bridge, osc_bridge);
                }
            });
    }
}
