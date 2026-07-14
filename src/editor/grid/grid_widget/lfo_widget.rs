use egui::Color32;

use crate::{
    editor::{
        grid::WidgetCtx,
        waveform::{self, WaveformOptions},
    },
    synth_engine::{
        Input, ModuleId, Sample,
        lfo::{Lfo, LfoShape, LfoUiBridge},
        ui_bridge::{ModuleBridge, UiBridge},
    },
};

use super::GridWidgetContent;

const PADDING: f32 = 4.0;
const DISPLAY_SAMPLES: usize = 256;
const WAVE_COLOR: Color32 = Color32::from_rgb(0x4a, 0xb0, 0xff);

pub struct LfoWidget {
    waveform: [Sample; DISPLAY_SAMPLES],
}

impl Default for LfoWidget {
    fn default() -> Self {
        Self {
            waveform: [0.0; DISPLAY_SAMPLES],
        }
    }
}

impl LfoWidget {
    fn build_waveform(
        &mut self,
        shape: LfoShape,
        phase_shift: Sample,
        skew: Sample,
        bipolar: bool,
    ) {
        let last = (DISPLAY_SAMPLES) as Sample;

        for (i, sample) in self.waveform.iter_mut().enumerate() {
            let t = i as Sample / last;
            *sample = Lfo::evaluate(shape, t, phase_shift, skew, bipolar);
        }
    }

    fn lfo_ui(
        &mut self,
        ui: &mut egui::Ui,
        bridge: &mut UiBridge,
        lfo_bridge: &mut LfoUiBridge,
        module_id: ModuleId,
    ) {
        let size = ui.available_size();
        let response = ui.allocate_response(size, egui::Sense::hover());
        let rect = response.rect.shrink2(egui::vec2(0.0, PADDING));

        if !rect.is_positive() || !ui.is_rect_visible(rect) {
            return;
        }

        let mut config = lfo_bridge.config().clone();

        bridge.apply_modulation(module_id, Input::PhaseShift, &mut config.phase_shift);
        bridge.apply_modulation(module_id, Input::Skew, &mut config.skew);

        self.build_waveform(
            config.shape,
            config.phase_shift[0],
            config.skew[0],
            config.bipolar,
        );

        waveform::paint_waveform_with_options(
            ui.painter(),
            rect,
            &self.waveform,
            WaveformOptions {
                loop_closed: false,
                normalize: false,
                color: WAVE_COLOR,
                fill: true,
                bipolar: config.bipolar,
            },
        );
    }
}

impl GridWidgetContent for LfoWidget {
    fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut WidgetCtx, module_id: ModuleId) {
        ctx.bridge
            .with_module_bridge(module_id, |bridge, module_bridge| {
                if let ModuleBridge::Lfo(lfo_bridge) = module_bridge {
                    self.lfo_ui(ui, bridge, lfo_bridge, module_id);
                }
            });
    }
}
