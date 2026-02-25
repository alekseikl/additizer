use egui_baseview::egui::{Checkbox, DragValue, Grid, Id, Modal, Sides, Ui};

use crate::{
    editor::{
        ModuleUi, db_slider::DbSlider, direct_input::DirectInput, gain_slider::GainSlider,
        modulation_input::ModulationInput, module_label::ModuleLabel, stereo_slider::StereoSlider,
        utils::confirm_module_removal,
    },
    show_modal,
    synth_engine::{
        Input, ModuleId, Sample, StereoSample, SynthEngine,
        oscillator::{Oscillator, OscillatorUIData, PhasesDst},
    },
};

struct GainShapeState {
    center: StereoSample,
    level: StereoSample, // dB
    to: bool,
}

struct RandomizePhaseState {
    from: Sample,
    to: Sample,
    stereo_spread: Sample,
    dst: PhasesDst,
}

pub struct OscillatorUI {
    module_id: ModuleId,
    remove_confirmation: bool,
    label_state: Option<String>,
    phases_shift_to: bool,
    gains_to: bool,
    gain_shape_state: Option<Box<GainShapeState>>,
    randomize_phase_state: Option<Box<RandomizePhaseState>>,
}

impl OscillatorUI {
    pub fn new(module_id: ModuleId) -> Self {
        Self {
            module_id,
            remove_confirmation: false,
            label_state: None,
            phases_shift_to: false,
            gains_to: false,
            gain_shape_state: None,
            randomize_phase_state: None,
        }
    }

    fn osc<'a>(&mut self, synth: &'a mut SynthEngine) -> &'a mut Oscillator {
        Oscillator::downcast_mut_unwrap(synth.get_module_mut(self.module_id))
    }

    fn show_gain_shape_modal(
        &mut self,
        synth: &mut SynthEngine,
        ui: &mut Ui,
        state: &mut GainShapeState,
    ) -> bool {
        let modal = Modal::new(Id::new("show_gain_shape_modal-modal")).show(ui.ctx(), |ui| {
            ui.heading("Levels shape");
            ui.add_space(20.0);
            ui.set_width(440.0);

            Grid::new("set-and-select-modal")
                .num_columns(2)
                .spacing([40.0, 24.0])
                .striped(true)
                .show(ui, |ui| {
                    ui.label("Center");
                    ui.add(
                        StereoSlider::new(&mut state.center)
                            .default_value(0.5)
                            .precision(2),
                    );
                    ui.end_row();

                    ui.label("Level");
                    ui.add(DbSlider::new(&mut state.level).max_dbs(6.0));
                    ui.end_row();
                });

            ui.add_space(40.0);

            Sides::new().show(
                ui,
                |_ui| {},
                |ui| {
                    if ui.button("Ok").clicked() {
                        self.osc(synth).apply_unison_level_shape(
                            state.center,
                            state.level,
                            state.to,
                        );
                        ui.close();
                    }

                    if ui.button("Apply").clicked() {
                        self.osc(synth).apply_unison_level_shape(
                            state.center,
                            state.level,
                            state.to,
                        );
                    }

                    if ui.button("Cancel").clicked() {
                        ui.close();
                    }
                },
            );
        });

        !modal.should_close()
    }

    fn show_randomize_phases_modal(
        &mut self,
        synth: &mut SynthEngine,
        ui: &mut Ui,
        state: &mut RandomizePhaseState,
    ) -> bool {
        let modal = Modal::new(Id::new("show_randomize_phases_modal")).show(ui.ctx(), |ui| {
            ui.heading("Randomize phases");
            ui.add_space(20.0);
            ui.set_width(440.0);

            let mut from = state.from.into();
            let mut to = state.to.into();
            let mut stereo_spread = state.stereo_spread.into();

            Grid::new("randomize_phases-grid")
                .num_columns(2)
                .spacing([40.0, 24.0])
                .striped(true)
                .show(ui, |ui| {
                    ui.label("From");
                    ui.add(StereoSlider::new(&mut from).default_value(0.0).precision(2));
                    ui.end_row();

                    ui.label("To");
                    ui.add(StereoSlider::new(&mut to).default_value(1.0).precision(2));
                    ui.end_row();

                    ui.label("Stereo spread");
                    ui.add(
                        StereoSlider::new(&mut stereo_spread)
                            .default_value(1.0)
                            .precision(2),
                    );
                    ui.end_row();
                });

            state.from = from.left();
            state.to = to.left();
            state.stereo_spread = stereo_spread.left();

            ui.add_space(40.0);

            Sides::new().show(
                ui,
                |_ui| {},
                |ui| {
                    if ui.button("Ok").clicked() {
                        self.osc(synth).randomize_phases(
                            state.from,
                            state.to,
                            state.stereo_spread,
                            state.dst,
                        );
                        ui.close();
                    }

                    if ui.button("Apply").clicked() {
                        self.osc(synth).randomize_phases(
                            state.from,
                            state.to,
                            state.stereo_spread,
                            state.dst,
                        );
                    }

                    if ui.button("Cancel").clicked() {
                        ui.close();
                    }
                },
            );
        });

        !modal.should_close()
    }

    fn show_phases(
        ui: &mut Ui,
        phases: impl Iterator<Item = StereoSample>,
    ) -> Option<(usize, StereoSample)> {
        let mut result = None;

        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                for (voice_idx, mut phase) in phases.enumerate() {
                    if ui
                        .add(
                            StereoSlider::new(&mut phase)
                                .vertical()
                                .thickness(12.0)
                                .length(100.0)
                                .precision(2)
                                .default_value(0.0),
                        )
                        .changed()
                    {
                        result = Some((voice_idx, phase));
                    }
                }
            });
        });

        result
    }

    fn show_gains(
        ui: &mut Ui,
        gains: impl Iterator<Item = StereoSample>,
    ) -> Option<(usize, StereoSample)> {
        let mut result = None;

        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                for (voice_idx, mut gain) in gains.enumerate() {
                    if ui
                        .add(
                            GainSlider::new(&mut gain)
                                .label(&format!("{}", voice_idx + 1))
                                .max_dbs(6.0)
                                .mid_point(0.8)
                                .skew(2.0)
                                .height(100.0),
                        )
                        .changed()
                    {
                        result = Some((voice_idx, gain));
                    }
                }
            });
        });

        result
    }

    fn show_unison_section(
        &mut self,
        ui_data: &mut OscillatorUIData,
        synth: &mut SynthEngine,
        ui: &mut Ui,
    ) {
        let params_iter = || ui_data.unison_params.iter().take(ui_data.unison);

        let from_to_toggle = |ui: &mut Ui, toggle: &mut bool| {
            if *toggle {
                if ui.button("From").clicked() {
                    *toggle = false;
                }
                ui.label("->");
                ui.label("To");
            } else {
                ui.label("From");
                ui.label("->");
                if ui.button("To").clicked() {
                    *toggle = true;
                }
            }
        };

        ui.label("Initial Phase");
        ui.vertical(|ui| {
            if let Some((voice_idx, phase)) =
                Self::show_phases(ui, params_iter().map(|p| p.initial_phase))
            {
                self.osc(synth).set_initial_phase(voice_idx, phase);
            }

            ui.add_space(8.0);

            if ui.button("Randomize").clicked() {
                self.randomize_phase_state = Some(Box::new(RandomizePhaseState {
                    from: 0.0,
                    to: 1.0,
                    stereo_spread: 0.1,
                    dst: PhasesDst::Initial,
                }));
            }
        });
        ui.end_row();

        ui.label("Phase Shift");
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                from_to_toggle(ui, &mut self.phases_shift_to);
            });

            ui.vertical(|ui| {
                if self.phases_shift_to {
                    if let Some((voice_idx, phase)) =
                        Self::show_phases(ui, params_iter().map(|p| p.phase_shift_to))
                    {
                        self.osc(synth).set_unison_phase_to(voice_idx, phase);
                    }
                } else if let Some((voice_idx, phase)) =
                    Self::show_phases(ui, params_iter().map(|p| p.phase_shift))
                {
                    self.osc(synth).set_unison_phase(voice_idx, phase);
                }

                ui.add_space(8.0);

                if ui.button("Randomize").clicked() {
                    self.randomize_phase_state = Some(Box::new(RandomizePhaseState {
                        from: 0.0,
                        to: 1.0,
                        stereo_spread: 0.1,
                        dst: if self.phases_shift_to {
                            PhasesDst::To
                        } else {
                            PhasesDst::From
                        },
                    }));
                }
            });
        });
        ui.end_row();

        ui.label("Phases Blend");
        if ui
            .add(ModulationInput::new(
                &mut ui_data.phases_blend,
                synth,
                Input::PhasesBlend,
                self.module_id,
            ))
            .changed()
        {
            self.osc(synth).set_phases_blend(ui_data.phases_blend);
        }
        ui.end_row();

        ui.label("Levels");
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                from_to_toggle(ui, &mut self.gains_to);
            });

            if self.gains_to {
                if let Some((voice_idx, gain)) =
                    Self::show_gains(ui, params_iter().map(|p| p.gain_to))
                {
                    self.osc(synth).set_unison_gain_to(voice_idx, gain);
                }
            } else if let Some((voice_idx, gain)) =
                Self::show_gains(ui, params_iter().map(|p| p.gain))
            {
                self.osc(synth).set_unison_gain(voice_idx, gain);
            }

            ui.add_space(8.0);

            if ui.button("Shape").clicked() {
                self.gain_shape_state = Some(Box::new(GainShapeState {
                    center: 0.5.into(),
                    level: (-24.0).into(),
                    to: self.gains_to,
                }));
            }
        });
        ui.end_row();

        ui.label("Levels Blend");
        if ui
            .add(ModulationInput::new(
                &mut ui_data.gains_blend,
                synth,
                Input::GainsBlend,
                self.module_id,
            ))
            .changed()
        {
            self.osc(synth).set_gains_blend(ui_data.gains_blend);
        }
        ui.end_row();
    }
}

impl ModuleUi for OscillatorUI {
    fn module_id(&self) -> Option<ModuleId> {
        Some(self.module_id)
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

                ui.label("Gain");
                if ui
                    .add(ModulationInput::new(
                        &mut ui_data.gain,
                        synth,
                        Input::Gain,
                        self.module_id,
                    ))
                    .changed()
                {
                    self.osc(synth).set_gain(ui_data.gain);
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

                ui.label("Frequency shift");
                if ui
                    .add(ModulationInput::new(
                        &mut ui_data.frequency_shift,
                        synth,
                        Input::FrequencyShift,
                        self.module_id,
                    ))
                    .changed()
                {
                    self.osc(synth).set_frequency_shift(ui_data.frequency_shift);
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

                ui.label("Detune power");
                if ui
                    .add(ModulationInput::new(
                        &mut ui_data.detune_power,
                        synth,
                        Input::DetunePower,
                        self.module_id,
                    ))
                    .changed()
                {
                    self.osc(synth).set_detune_power(ui_data.detune_power);
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

                ui.label("Unison");
                if ui
                    .add(DragValue::new(&mut ui_data.unison).range(1..=16))
                    .changed()
                {
                    self.osc(synth).set_unison(ui_data.unison);
                }
                ui.end_row();

                if ui_data.unison > 1 {
                    self.show_unison_section(&mut ui_data, synth, ui);
                }
            });

        ui.add_space(40.0);

        show_modal!(self, gain_shape_state, show_gain_shape_modal, synth, ui);
        show_modal!(
            self,
            randomize_phase_state,
            show_randomize_phases_modal,
            synth,
            ui
        );

        if confirm_module_removal(ui, &mut self.remove_confirmation) {
            synth.remove_module(self.module_id);
        }
    }
}
