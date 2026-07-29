use egui::{Grid, Ui};

use crate::{
    editor::{
        direct_input::DirectInput, module_label::ModuleLabel, stereo_input::StereoInput,
        utils::confirm_module_removal, ModuleUi,
    },
    synth_engine::{
        spectral_blend::SpectralBlendUiBridge,
        ui_bridge::{ModuleBridge, UiBridge},
        Input, ModuleId,
    },
};

pub struct SpectralBlendUi {
    module_id: ModuleId,
    remove_confirmation: bool,
    label_state: Option<String>,
}

impl SpectralBlendUi {
    pub fn new(module_id: ModuleId) -> Self {
        Self {
            module_id,
            remove_confirmation: false,
            label_state: None,
        }
    }

    fn paint_ui(
        &mut self,
        bridge: &mut UiBridge,
        blend_bridge: &mut SpectralBlendUiBridge,
        ui: &mut Ui,
    ) {
        let module_id = self.module_id;
        let mut config = blend_bridge.config().clone();

        ui.add(ModuleLabel::new(&mut self.label_state, bridge, module_id));

        ui.add_space(20.0);

        Grid::new("spectral_blend_grid")
            .num_columns(2)
            .spacing([40.0, 24.0])
            .striped(true)
            .show(ui, |ui| {
                ui.label("From");
                ui.add(DirectInput::new(bridge, Input::Spectrum, module_id));
                ui.end_row();

                ui.label("To");
                ui.add(DirectInput::new(bridge, Input::SpectrumTo, module_id));
                ui.end_row();

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

        ui.add_space(40.0);

        if confirm_module_removal(ui, &mut self.remove_confirmation) {
            bridge.remove_module(module_id);
        }
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
