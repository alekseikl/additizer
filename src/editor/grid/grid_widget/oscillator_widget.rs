use std::sync::Arc;

use realfft::{ComplexToReal, RealFftPlanner};

use crate::{
    editor::{grid::WidgetCtx, waveform},
    synth_engine::{
        ComplexSample, ModuleId, Sample,
        oscillator::{DISPLAY_SPECTRUM_SIZE, OscillatorUiBridge},
        ui_bridge::{ModuleBridge, UiBridge},
    },
};

use super::GridWidgetContent;

const WAVE_PADDING: f32 = 4.0;
const DISPLAY_WAVEFORM_SIZE: usize = DISPLAY_SPECTRUM_SIZE * 2;
const DISPLAY_DFT_SIZE: usize = DISPLAY_WAVEFORM_SIZE / 2 + 1;

pub struct OscillatorWidget {
    inverse_fft: Arc<dyn ComplexToReal<Sample>>,
    dft_buff: Box<[ComplexSample; DISPLAY_DFT_SIZE]>,
    scratch_buff: Box<[ComplexSample; DISPLAY_DFT_SIZE]>,
    waveform: Box<[Sample; DISPLAY_WAVEFORM_SIZE]>,
}

impl Default for OscillatorWidget {
    fn default() -> Self {
        Self {
            inverse_fft: RealFftPlanner::<Sample>::new().plan_fft_inverse(DISPLAY_WAVEFORM_SIZE),
            dft_buff: Box::new([ComplexSample::ZERO; DISPLAY_DFT_SIZE]),
            scratch_buff: Box::new([ComplexSample::ZERO; DISPLAY_DFT_SIZE]),
            waveform: Box::new([0.0; DISPLAY_WAVEFORM_SIZE]),
        }
    }
}

impl OscillatorWidget {
    fn build_waveform(&mut self, spectrum: &[ComplexSample; DISPLAY_SPECTRUM_SIZE]) {
        self.dft_buff[..DISPLAY_SPECTRUM_SIZE].copy_from_slice(spectrum);
        self.dft_buff[DISPLAY_SPECTRUM_SIZE..].fill(ComplexSample::ZERO);

        self.inverse_fft
            .process_with_scratch(
                self.dft_buff.as_mut_slice(),
                self.waveform.as_mut_slice(),
                self.scratch_buff.as_mut_slice(),
            )
            .unwrap();
    }

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
            self.build_waveform(osc_bridge.get_spectrum());
            waveform::paint_waveform(painter, rect, self.waveform.as_slice());
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
