use crate::{
    editor::{grid::WidgetCtx, volume_meter::VolumeMeter},
    synth_engine::{
        ModuleId, StereoSample,
        mixer::MixerUiBridge,
        ui_bridge::{GridVec, ModuleBridge},
    },
};

use super::GridWidgetContent;

const PADDING: f32 = 4.0;

#[derive(Default)]
pub struct MixerWidget {
    volume_meter: VolumeMeter,
}

impl MixerWidget {
    fn mixer_ui(
        &mut self,
        ui: &mut egui::Ui,
        has_active_voices: bool,
        mixer_bridge: &mut MixerUiBridge,
    ) {
        let size = ui.available_size();
        let response = ui.allocate_response(size, egui::Sense::hover());
        let rect = response.rect.shrink2(egui::vec2(0.0, PADDING));

        if !rect.is_positive() || !ui.is_rect_visible(rect) {
            return;
        }

        let volume = if has_active_voices {
            mixer_bridge.get_out_volume()
        } else {
            StereoSample::ZERO
        };

        self.volume_meter
            .paint_stereo(&ui.painter().with_clip_rect(rect), rect, volume);
    }
}

impl GridWidgetContent for MixerWidget {
    fn grid_size(&self) -> GridVec {
        GridVec::new(3, 2)
    }

    fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut WidgetCtx, module_id: ModuleId) {
        let has_active_voices = ctx.bridge.has_active_voices();

        ctx.bridge
            .with_module_bridge(module_id, |_bridge, module_bridge| {
                if let ModuleBridge::Mixer(mixer_bridge) = module_bridge {
                    self.mixer_ui(ui, has_active_voices, mixer_bridge);
                }
            });
    }
}
