use egui::{Grid, Ui};

use crate::{
    editor::{module_label::ModuleLabel, stereo_input::StereoInput, ModuleUi},
    synth_engine::{
        spectral_blend::SpectralBlendUiBridge,
        ui_bridge::{ModuleBridge, UiBridge},
        Input, ModuleId, ModuleType,
    },
};

pub struct SpectralBlendUi {
    module_id: ModuleId,
}

impl SpectralBlendUi {
    pub fn new(module_id: ModuleId) -> Self {
        Self { module_id }
    }

    fn paint_ui(
        &mut self,
        bridge: &mut UiBridge,
        blend_bridge: &mut SpectralBlendUiBridge,
        ui: &mut Ui,
    ) {
        let module_id = self.module_id;
        let mut config = blend_bridge.config().clone();

        ui.add(ModuleLabel::new(module_id, ModuleType::SpectralBlend, bridge));

        ui.add_space(16.0);

        Grid::new("spectral_blend_grid")
            .num_columns(2)
            .spacing([40.0, 24.0])
            .striped(true)
            .show(ui, |ui| {
                ui.label("Blend");
                if ui
                    .add(StereoInput::new(
                        Input::Blend,
                        module_id,
                        &mut config.blend,
                        bridge,
                    ))
                    .changed()
                {
                    blend_bridge.set_param(Input::Blend, config.blend);
                }
                ui.end_row();
            });
    }
}

impl ModuleUi for SpectralBlendUi {
    fn module_id(&self) -> Option<ModuleId> {
        Some(self.module_id)
    }

    fn ui(&mut self, bridge: &mut UiBridge, ui: &mut Ui) {
        bridge.with_module_bridge(self.module_id, |bridge, module_bridge| {
            if let ModuleBridge::SpectralBlend(blend_bridge) = module_bridge {
                self.paint_ui(bridge, blend_bridge, ui);
            }
        });
    }
}
