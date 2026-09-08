use egui::{Grid, Ui};

use crate::{
    editor::{ModuleUi, module_label::ModuleLabel, stereo_input::StereoInput},
    synth_engine::{
        Input, ModuleId, ModuleType,
        amplifier::AmplifierUiBridge,
        ui_bridge::{ModuleBridge, UiBridge},
    },
};

pub struct AmplifierUI {
    module_id: ModuleId,
}

impl AmplifierUI {
    pub fn new(module_id: ModuleId) -> Self {
        Self { module_id }
    }

    fn paint_ui(&mut self, bridge: &mut UiBridge, amp_bridge: &mut AmplifierUiBridge, ui: &mut Ui) {
        let module_id = self.module_id;
        let mut config = amp_bridge.config().clone();

        ui.add(ModuleLabel::new(module_id, ModuleType::Amplifier, bridge));

        ui.add_space(16.0);

        Grid::new("amp_grid")
            .num_columns(4)
            .spacing([8.0, 8.0])
            .show(ui, |ui| {
                ui.label("Gain");
                if ui
                    .add(
                        StereoInput::new(Input::Gain, module_id, &mut config.gain, bridge)
                            .default(0.0),
                    )
                    .changed()
                {
                    amp_bridge.set_param(Input::Gain, config.gain);
                }

                ui.label("Level");
                if ui
                    .add(
                        StereoInput::new(Input::Level, module_id, &mut config.level, bridge)
                            .default(0.0),
                    )
                    .changed()
                {
                    amp_bridge.set_param(Input::Level, config.level);
                }
                ui.end_row();

                ui.label("Pan");
                if ui
                    .add(StereoInput::new(
                        Input::Pan,
                        module_id,
                        &mut config.pan,
                        bridge,
                    ))
                    .changed()
                {
                    amp_bridge.set_param(Input::Pan, config.pan);
                }
                ui.end_row();
            });
    }
}

impl ModuleUi for AmplifierUI {
    fn module_id(&self) -> Option<ModuleId> {
        Some(self.module_id)
    }

    fn ui(&mut self, bridge: &mut UiBridge, ui: &mut Ui) {
        bridge.with_module_bridge(self.module_id, |bridge, module_bridge| {
            if let ModuleBridge::Amplifier(amp_bridge) = module_bridge {
                self.paint_ui(bridge, amp_bridge, ui);
            }
        });
    }
}
