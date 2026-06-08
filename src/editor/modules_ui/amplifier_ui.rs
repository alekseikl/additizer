use egui::{Grid, Ui};

use crate::{
    editor::{
        ModuleUi, modulation_input::ModulationInput, module_label::ModuleLabel,
        multi_input::MultiInput, utils::confirm_module_removal,
    },
    synth_engine::{Input, ModuleId, amplifier, ui_bridge::UiBridge},
};

pub struct AmplifierUI {
    remove_confirmation: bool,
    module_label: Option<String>,
    amp_bridge: amplifier::UiBridge,
}

impl AmplifierUI {
    pub fn new(module_id: ModuleId, synth_bridge: &mut UiBridge) -> Option<Self> {
        let amp_bridge = amplifier::UiBridge::create(module_id, synth_bridge.engine().clone())?;

        Some(Self {
            remove_confirmation: false,
            module_label: None,
            amp_bridge,
        })
    }
}

impl ModuleUi for AmplifierUI {
    fn module_id(&self) -> Option<ModuleId> {
        Some(self.amp_bridge.module_id())
    }

    fn ui(&mut self, bridge: &mut UiBridge, ui: &mut Ui) {
        let module_id = self.amp_bridge.module_id();
        let mut config = self.amp_bridge.config().clone();

        ui.add(ModuleLabel::new(&mut self.module_label, bridge, module_id));

        ui.add_space(20.0);

        Grid::new("amp_grid")
            .num_columns(2)
            .spacing([40.0, 24.0])
            .striped(true)
            .show(ui, |ui| {
                ui.label("Inputs");
                MultiInput::new(Input::Audio, module_id).show(ui, bridge);
                ui.end_row();

                ui.label("Gain");
                if ui
                    .add(
                        ModulationInput::new(&mut config.gain, bridge, Input::Gain, module_id)
                            .modulation_default(1.0),
                    )
                    .changed()
                {
                    self.amp_bridge.set_param(Input::Gain, config.gain);
                }
                ui.end_row();
            });

        ui.add_space(40.0);

        if confirm_module_removal(ui, &mut self.remove_confirmation) {
            bridge.remove_module(module_id);
        }
    }
}
