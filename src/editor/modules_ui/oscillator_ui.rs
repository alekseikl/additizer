use egui_baseview::egui::{Checkbox, DragValue, Grid, Ui};

use crate::{
    editor::{
        ModuleUI, direct_input::DirectInput, modulation_input::ModulationInput,
        module_label::ModuleLabel, stereo_slider::StereoSlider, utils::confirm_module_removal,
    },
    synth_engine::{Input, ModuleId, Oscillator, SynthEngine},
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
                ui.add(DirectInput::new(synth, Input::Spectrum, self.module_id));
                ui.end_row();

                ui.label("Level");
                if ui
                    .add(ModulationInput::new(
                        &mut ui_data.level,
                        synth,
                        Input::Level,
                        self.module_id,
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
                        Input::PitchShift,
                        self.module_id,
                    ))
                    .changed()
                {
                    self.osc(synth).set_pitch_shift(ui_data.pitch_shift);
                }
                ui.end_row();

                ui.label("Phase shift");
                if ui
                    .add(ModulationInput::new(
                        &mut ui_data.phase_shift,
                        synth,
                        Input::PhaseShift,
                        self.module_id,
                    ))
                    .changed()
                {
                    self.osc(synth).set_phase_shift(ui_data.phase_shift);
                }
                ui.end_row();

                ui.label("Detune");
                if ui
                    .add(ModulationInput::new(
                        &mut ui_data.detune,
                        synth,
                        Input::Detune,
                        self.module_id,
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

                ui.label("Reset phase");
                if ui
                    .add(Checkbox::without_text(&mut ui_data.reset_phase))
                    .changed()
                {
                    self.osc(synth).set_reset_phase(ui_data.reset_phase);
                }
                ui.end_row();

                ui.label("Initial Phases");
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        let phase = &mut ui_data.initial_phases[0];

                        ui.label("#1");
                        if ui
                            .add(StereoSlider::new(phase).default_value(0.0).precision(2))
                            .changed()
                        {
                            self.osc(synth).set_initial_phase(0, *phase);
                        }
                    });
                    ui.add_space(8.0);

                    ui.collapsing("Unison Phases", |ui| {
                        for (idx, phase) in ui_data.initial_phases[1..].iter_mut().enumerate() {
                            ui.horizontal(|ui| {
                                ui.label(format!("#{}", idx + 2));
                                if ui
                                    .add(StereoSlider::new(phase).default_value(0.0).precision(2))
                                    .changed()
                                {
                                    self.osc(synth).set_initial_phase(idx + 1, *phase);
                                }
                            });
                        }
                    });
                });
                ui.end_row();
            });

        ui.add_space(40.0);

        if confirm_module_removal(ui, &mut self.remove_confirmation) {
            synth.remove_module(self.module_id);
        }
    }
}
