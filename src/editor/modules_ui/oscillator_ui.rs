use egui_baseview::egui::{Checkbox, DragValue, Grid, Response, Ui, Widget};

use crate::{
    editor::{direct_input::DirectInput, modulation_input::ModulationInput},
    synth_engine::{ModuleId, ModuleInput, Oscillator, SynthEngine},
};

pub struct OscillatorUI<'a> {
    module_id: ModuleId,
    synth_engine: &'a mut SynthEngine,
}

impl<'a> OscillatorUI<'a> {
    pub fn new(module_id: ModuleId, synth_engine: &'a mut SynthEngine) -> Self {
        Self {
            module_id,
            synth_engine,
        }
    }

    fn osc(&mut self) -> &mut Oscillator {
        Oscillator::downcast_mut_unwrap(self.synth_engine.get_module_mut(self.module_id))
    }
}

impl Widget for OscillatorUI<'_> {
    fn ui(mut self, ui: &mut Ui) -> Response {
        let mut ui_data = self.osc().get_ui();

        ui.heading("Oscillator");
        ui.add_space(20.0);

        Grid::new("osc_grid")
            .num_columns(2)
            .spacing([40.0, 24.0])
            .striped(true)
            .show(ui, |ui| {
                ui.label("Input");
                ui.add(DirectInput::new(
                    self.synth_engine,
                    ModuleInput::spectrum(self.module_id),
                ));
                ui.end_row();

                ui.label("Level");
                if ui
                    .add(ModulationInput::new(
                        &mut ui_data.level,
                        self.synth_engine,
                        ModuleInput::level(self.module_id),
                    ))
                    .changed()
                {
                    self.osc().set_level(ui_data.level);
                }
                ui.end_row();

                ui.label("Pitch shift");
                if ui
                    .add(ModulationInput::new(
                        &mut ui_data.pitch_shift,
                        self.synth_engine,
                        ModuleInput::pitch_shift(self.module_id),
                    ))
                    .changed()
                {
                    self.osc().set_pitch_shift(ui_data.pitch_shift);
                }
                ui.end_row();

                ui.label("Detune");
                if ui
                    .add(ModulationInput::new(
                        &mut ui_data.detune,
                        self.synth_engine,
                        ModuleInput::detune(self.module_id),
                    ))
                    .changed()
                {
                    self.osc().set_detune(ui_data.detune);
                }
                ui.end_row();

                ui.label("Unison");
                if ui
                    .add(DragValue::new(&mut ui_data.unison).range(1..=16))
                    .changed()
                {
                    self.osc().set_unison(ui_data.unison);
                }
                ui.end_row();

                ui.label("Same note phases");
                if ui
                    .add(Checkbox::without_text(&mut ui_data.same_channel_phases))
                    .changed()
                {
                    self.osc()
                        .set_same_channels_phases(ui_data.same_channel_phases);
                }
                ui.end_row();
            })
            .response
    }
}
