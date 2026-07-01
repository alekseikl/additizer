use crate::{
    editor::{grid::WidgetCtx, waveform},
    synth_engine::{
        ModuleId,
        oscillator::OscillatorUiBridge,
        ui_bridge::{ModuleBridge, UiBridge},
    },
};

use super::GridWidgetContent;

const WAVE_PADDING: f32 = 4.0;

pub struct OscillatorWidget {}

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
            let waveform = osc_bridge.get_waveform();

            waveform::paint_waveform(painter, rect, waveform);
        }

        // Keep the preview live while a voice is being processed.
        ui.ctx().request_repaint();
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
