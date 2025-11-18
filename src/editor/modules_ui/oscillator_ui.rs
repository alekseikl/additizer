use egui_baseview::egui::{Checkbox, DragValue, Grid, Ui};

use crate::{
    editor::{
        ModuleUI, direct_input::DirectInput, modulation_input::ModulationInput,
        module_label::ModuleLabel, utils::confirm_module_removal,
    },
    synth_engine::{ModuleId, ModuleInput, Oscillator, SynthEngine},
};

pub struct OscillatorUI {
    module_id: ModuleId,
    remove_confirmation: bool,
    label_state: Option<String>,
}

impl OscillatorUI {
    pub fn new(module_id: ModuleId) -> Self {
        Self {
            module_id,
            remove_confirmation: false,
            label_state: None,
        }
    }

    fn osc<'a>(&mut self, synth: &'a mut SynthEngine) -> &'a mut Oscillator {
        Oscillator::downcast_mut_unwrap(synth.get_module_mut(self.module_id))
    }
}

impl ModuleUI for OscillatorUI {
    fn module_id(&self) -> ModuleId {
        self.module_id
    }

    fn ui(&mut self, synth: &mut SynthEngine, ui: &mut Ui) {
        let mut ui_data = self.osc(synth).get_ui();

        ui.add(ModuleLabel::new(
            &ui_data.label,
            &mut self.label_state,
            synth.get_module_mut(self.module_id).unwrap(),
        ));

        ui.add_space(20.0);

        Grid::new("osc_grid")
            .num_columns(2)
            .spacing([40.0, 24.0])
            .striped(true)
            .show(ui, |ui| {
                ui.label("Input");
                ui.add(DirectInput::new(
                    synth,
                    ModuleInput::spectrum(self.module_id),
                ));
                ui.end_row();

                ui.label("Level");
                if ui
                    .add(ModulationInput::new(
                        &mut ui_data.level,
                        synth,
                        ModuleInput::level(self.module_id),
                    ))
                    .changed()
                {
                    self.osc(synth).set_level(ui_data.level);
                }
                ui.end_row();

                ui.label("Pitch shift");
                if ui
                    .add(ModulationInput::new(
                        &mut ui_data.pitch_shift,
                        synth,
                        ModuleInput::pitch_shift(self.module_id),
                    ))
                    .changed()
                {
                    self.osc(synth).set_pitch_shift(ui_data.pitch_shift);
                }
                ui.end_row();

                ui.label("Detune");
                if ui
                    .add(ModulationInput::new(
                        &mut ui_data.detune,
                        synth,
                        ModuleInput::detune(self.module_id),
                    ))
                    .changed()
                {
                    self.osc(synth).set_detune(ui_data.detune);
                }
                ui.end_row();

                ui.label("Unison");
                if ui
                    .add(DragValue::new(&mut ui_data.unison).range(1..=16))
                    .changed()
                {
                    self.osc(synth).set_unison(ui_data.unison);
                }
                ui.end_row();

                ui.label("Same note phases");
                if ui
                    .add(Checkbox::without_text(&mut ui_data.same_channel_phases))
                    .changed()
                {
                    self.osc(synth)
                        .set_same_channels_phases(ui_data.same_channel_phases);
                }
                ui.end_row();
            });

        ui.add_space(40.0);

        if confirm_module_removal(ui, &mut self.remove_confirmation) {
            synth.remove_module(self.module_id);
        }
    }
}
