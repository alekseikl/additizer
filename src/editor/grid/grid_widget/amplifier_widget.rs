use crate::{
    editor::{
        grid::WidgetCtx,
        stereo_smoother::StereoSmoother,
        volume_meter,
    },
    synth_engine::{
        ModuleId, Sample, StereoSample,
        amplifier::AmplifierUiBridge,
        ui_bridge::{GridVec, ModuleBridge},
    },
};

use super::GridWidgetContent;

const PADDING: f32 = 4.0;
const VOLUME_SMOOTH_TIME: Sample = 0.15;

pub struct AmplifierWidget {
    volume_smoother: StereoSmoother,
}

impl Default for AmplifierWidget {
    fn default() -> Self {
        Self {
            volume_smoother: StereoSmoother::new(StereoSample::ZERO, VOLUME_SMOOTH_TIME),
        }
    }
}

impl AmplifierWidget {
    fn amplifier_ui(
        &mut self,
        ui: &mut egui::Ui,
        has_active_voices: bool,
        amp_bridge: &mut AmplifierUiBridge,
    ) {
        let size = ui.available_size();
        let response = ui.allocate_response(size, egui::Sense::hover());
        let rect = response.rect.shrink2(egui::vec2(0.0, PADDING));

        if !rect.is_positive() || !ui.is_rect_visible(rect) {
            return;
        }

        let volume_target = if has_active_voices {
            amp_bridge.get_out_volume()
        } else {
            StereoSample::ZERO
        };
        let volume = self.volume_smoother.tick(volume_target);

        volume_meter::paint_stereo(&ui.painter().with_clip_rect(rect), rect, volume);
    }
}

impl GridWidgetContent for AmplifierWidget {
    fn grid_size(&self) -> GridVec {
        GridVec::new(3, 2)
    }

    fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut WidgetCtx, module_id: ModuleId) {
        let has_active_voices = ctx.bridge.has_active_voices();

        ctx.bridge
            .with_module_bridge(module_id, |_bridge, module_bridge| {
                if let ModuleBridge::Amplifier(amp_bridge) = module_bridge {
                    self.amplifier_ui(ui, has_active_voices, amp_bridge);
                }
            });
    }
}
