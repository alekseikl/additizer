use egui_baseview::egui::{Grid, Response, Ui, Widget};

use crate::{
    editor::{direct_input::DirectInput, modulation_input::ModulationInput},
    synth_engine::{Amplifier, ModuleId, ModuleInput, SynthEngine},
};

pub struct AmplifierUI<'a> {
    module_id: ModuleId,
    synth_engine: &'a mut SynthEngine,
}

impl<'a> AmplifierUI<'a> {
    pub fn new(module_id: ModuleId, synth_engine: &'a mut SynthEngine) -> Self {
        Self {
            module_id,
            synth_engine,
        }
    }

    fn amp(&mut self) -> &mut Amplifier {
        Amplifier::downcast_mut_unwrap(self.synth_engine.get_module_mut(self.module_id))
    }
}

impl Widget for AmplifierUI<'_> {
    fn ui(mut self, ui: &mut Ui) -> Response {
        let mut ui_data = self.amp().get_ui();

        ui.heading("Amplifier");
        ui.add_space(20.0);

        Grid::new("amp_grid")
            .num_columns(2)
            .spacing([40.0, 24.0])
            .striped(true)
            .show(ui, |ui| {
                ui.label("Input");
                ui.add(DirectInput::new(
                    self.synth_engine,
                    ModuleInput::audio(self.module_id),
                ));
                ui.end_row();

                ui.label("Level");
                if ui
                    .add(
                        ModulationInput::new(
                            &mut ui_data.level,
                            self.synth_engine,
                            ModuleInput::level(self.module_id),
                        )
                        .modulation_default(1.0),
                    )
                    .changed()
                {
                    self.amp().set_level(ui_data.level);
                }
                ui.end_row();
            })
            .response
    }
}
