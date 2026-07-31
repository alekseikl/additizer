use egui::Label;

use crate::{
    editor::{grid::WidgetCtx, volume_meter::VolumeMeter},
    synth_engine::{
        ModuleId, StereoSample,
        amplifier::AmplifierUiBridge,
        ui_bridge::{GridVec, ModuleBridge},
    },
};

use super::GridWidgetContent;

const PADDING: f32 = 4.0;

#[derive(Default)]
pub struct AmplifierWidget {
    volume_meter: VolumeMeter,
}

impl AmplifierWidget {
    fn amplifier_ui(
        &mut self,
        ui: &mut egui::Ui,
        has_active_voices: bool,
        label: String,
        amp_bridge: &mut AmplifierUiBridge,
    ) {
        ui.add_space(2.0);
        ui.add(Label::new("Amp").selectable(false).truncate())
            .on_hover_text(label);

        let size = ui.available_size();
        let response = ui.allocate_response(size, egui::Sense::hover());
        let rect = response.rect.shrink2(egui::vec2(0.0, PADDING));

        if !rect.is_positive() || !ui.is_rect_visible(rect) {
            return;
        }

        let volume = if has_active_voices {
            amp_bridge.get_out_volume()
        } else {
            StereoSample::ZERO
        };

        self.volume_meter
            .paint_stereo(&ui.painter().with_clip_rect(rect), rect, volume);
    }
}

impl GridWidgetContent for AmplifierWidget {
    fn grid_size(&self) -> GridVec {
        GridVec::new(2, 2)
    }

    fn show_label(&self) -> bool {
        false
    }

    fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut WidgetCtx, module_id: ModuleId) {
        let has_active_voices = ctx.bridge.has_active_voices();
        let label = ctx.bridge.display_module_label(module_id);

        ctx.bridge
            .with_module_bridge(module_id, |_bridge, module_bridge| {
                if let ModuleBridge::Amplifier(amp_bridge) = module_bridge {
                    self.amplifier_ui(ui, has_active_voices, label, amp_bridge);
                }
            });
    }
}
