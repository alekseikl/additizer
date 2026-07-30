use egui::{Grid, Ui};

use crate::{
    editor::{ModuleUi, module_label::ModuleLabel, stereo_input::StereoInput},
    synth_engine::{
        Input, ModuleId,
        amplifier::AmplifierUiBridge,
        ui_bridge::{ModuleBridge, UiBridge},
    },
};

pub struct AmplifierUI {
    module_id: ModuleId,
    module_label: Option<String>,
}

impl AmplifierUI {
    pub fn new(module_id: ModuleId) -> Self {
        Self {
            module_id,
            module_label: None,
        }
    }

    fn paint_ui(&mut self, bridge: &mut UiBridge, amp_bridge: &mut AmplifierUiBridge, ui: &mut Ui) {
        let module_id = self.module_id;
        let mut config = amp_bridge.config().clone();

        ui.add(ModuleLabel::new(&mut self.module_label, bridge, module_id));

        ui.add_space(20.0);

        Grid::new("amp_grid")
            .num_columns(2)
            .spacing([40.0, 24.0])
            .striped(true)
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
