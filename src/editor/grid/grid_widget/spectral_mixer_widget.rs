use crate::{
    editor::{gain_bars, grid::WidgetCtx},
    synth_engine::{
        Input, ModuleId, spectral_mixer::SpectralMixerUiBridge, ui_bridge::ModuleBridge,
    },
};

use super::GridWidgetContent;

pub struct SpectralMixerWidget {}

impl SpectralMixerWidget {
    fn mixer_ui(&mut self, ui: &mut egui::Ui, mixer_bridge: &mut SpectralMixerUiBridge) {
        gain_bars::paint_gain_bars(ui, mixer_bridge.get_spectrum());
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

    fn input_label(&self, input: Input) -> String {
        match input {
            Input::Gain => "Output gain".into(),
            Input::Level => "Output level (dB)".into(),
            Input::SpectrumMix(i) => format!("Spectrum In #{}", i + 1),
            Input::GainMix(i) => format!("Input #{} gain ", i + 1),
            Input::LevelMix(i) => format!("Input #{} level (dB) ", i + 1),
            _ => input.label(),
        }
    }
}
