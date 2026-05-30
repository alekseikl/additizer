use egui::{Checkbox, DragValue, Grid, Id, Modal, Sides, Ui};

use crate::{
    editor::{
        ModuleUi, SynthEngineHandle, db_slider::DbSlider, direct_input::DirectInput,
        gain_slider::GainSlider, modulation_input::ModulationInput, module_label::ModuleLabel,
        stereo_slider::StereoSlider, utils::confirm_module_removal,
    },
    synth_engine::{
        Input, ModuleId, Sample, StereoSample, SynthModule,
        oscillator::{AudioBridge, Oscillator, PhasesDst, UiState, UiUpdate},
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
    label: String,
    ui_state: UiState,
    bridge: Option<AudioBridge>,
    phases_shift_to: bool,
    gains_to: bool,
    gain_shape_state: Option<Box<GainShapeState>>,
    randomize_phase_state: Option<Box<RandomizePhaseState>>,
}

impl OscillatorUI {
    pub fn new(module_id: ModuleId, synth: &SynthEngineHandle) -> Self {
        let mut s = synth.lock();
        let osc = s.get_typed_module_mut::<Oscillator>(module_id).unwrap();

        Self {
            module_id,
            remove_confirmation: false,
            label_state: None,
            label: osc.label(),
            ui_state: osc.get_ui_state(),
            bridge: osc.take_audio_bridge(),
            phases_shift_to: false,
            gains_to: false,
            gain_shape_state: None,
            randomize_phase_state: None,
        }
    }

    fn apply_ui_update(ui_state: &mut UiState, update: UiUpdate) {
        match update {
            UiUpdate::ModulatedInput {
                input,
                channel,
                value,
            } => {
                let channel = channel as usize;
                match input {
                    Input::Gain => ui_state.gain[channel] = value,
                    Input::PitchShift => ui_state.pitch_shift[channel] = value,
                    Input::PhaseShift => ui_state.phase_shift[channel] = value,
                    Input::FrequencyShift => ui_state.frequency_shift[channel] = value,
                    Input::Detune => ui_state.detune[channel] = value,
                    Input::DetunePower => ui_state.detune_power[channel] = value,
                    Input::Glide => ui_state.glide[channel] = value,
                    Input::GlideSlope => ui_state.glide_slope[channel] = value,
                    Input::PhasesBlend => ui_state.phases_blend[channel] = value,
                    Input::GainsBlend => ui_state.gains_blend[channel] = value,
                    _ => (),
                }
            }
            UiUpdate::Output { .. } => (),
            UiUpdate::RefreshState => (),
        }
    }

    fn show_gain_shape_modal(
        &mut self,
        bridge: &mut AudioBridge,
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
                        bridge.apply_unison_level_shape(state.center, state.level, state.to);
                        ui.close();
                    }

                    if ui.button("Apply").clicked() {
                        bridge.apply_unison_level_shape(state.center, state.level, state.to);
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
        bridge: &mut AudioBridge,
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
                        bridge.randomize_phases(
                            state.from,
                            state.to,
                            state.stereo_spread,
                            state.dst,
                        );
                        ui.close();
                    }

                    if ui.button("Apply").clicked() {
                        bridge.randomize_phases(
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
        bridge: &mut AudioBridge,
        synth: &SynthEngineHandle,
        ui: &mut Ui,
    ) {
        let unison = self.ui_state.unison;

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
            if let Some((voice_idx, phase)) = Self::show_phases(
                ui,
                (0..unison).map(|i| self.ui_state.unison_params[i].initial_phase),
            ) {
                self.ui_state.unison_params[voice_idx].initial_phase = phase;
                bridge.set_unison_initial_phase(voice_idx, phase);
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
                    if let Some((voice_idx, phase)) = Self::show_phases(
                        ui,
                        (0..unison).map(|i| self.ui_state.unison_params[i].phase_shift_to),
                    ) {
                        self.ui_state.unison_params[voice_idx].phase_shift_to = phase;
                        bridge.set_unison_phase_shift_to(voice_idx, phase);
                    }
                } else if let Some((voice_idx, phase)) = Self::show_phases(
                    ui,
                    (0..unison).map(|i| self.ui_state.unison_params[i].phase_shift),
                ) {
                    self.ui_state.unison_params[voice_idx].phase_shift = phase;
                    bridge.set_unison_phase_shift(voice_idx, phase);
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
                &mut self.ui_state.phases_blend,
                synth.clone(),
                Input::PhasesBlend,
                self.module_id,
            ))
            .changed()
        {
            bridge.set_param(Input::PhasesBlend, self.ui_state.phases_blend);
        }
        ui.end_row();

        ui.label("Levels");
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                from_to_toggle(ui, &mut self.gains_to);
            });

            if self.gains_to {
                if let Some((voice_idx, gain)) = Self::show_gains(
                    ui,
                    (0..unison).map(|i| self.ui_state.unison_params[i].gain_to),
                ) {
                    self.ui_state.unison_params[voice_idx].gain_to = gain;
                    bridge.set_unison_gain_to(voice_idx, gain);
                }
            } else if let Some((voice_idx, gain)) =
                Self::show_gains(ui, (0..unison).map(|i| self.ui_state.unison_params[i].gain))
            {
                self.ui_state.unison_params[voice_idx].gain = gain;
                bridge.set_unison_gain(voice_idx, gain);
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
                &mut self.ui_state.gains_blend,
                synth.clone(),
                Input::GainsBlend,
                self.module_id,
            ))
            .changed()
        {
            bridge.set_param(Input::GainsBlend, self.ui_state.gains_blend);
        }
        ui.end_row();
    }
}

impl ModuleUi for OscillatorUI {
    fn module_id(&self) -> Option<ModuleId> {
        Some(self.module_id)
    }

    fn ui(&mut self, synth: &SynthEngineHandle, ui: &mut Ui) {
        let mut bridge = self.bridge.take().unwrap();
        let mut refresh_state = false;

        while let Some(update) = bridge.pop_update() {
            if matches!(update, UiUpdate::RefreshState) {
                refresh_state = true;
            } else {
                Self::apply_ui_update(&mut self.ui_state, update);
            }
        }

        if refresh_state {
            self.ui_state = synth
                .lock()
                .get_typed_module_mut::<Oscillator>(self.module_id)
                .unwrap()
                .get_ui_state();
        }

        ui.add(ModuleLabel::new(
            &self.label,
            &mut self.label_state,
            synth,
            self.module_id,
        ));

        ui.add_space(20.0);

        Grid::new("osc_grid")
            .num_columns(2)
            .spacing([40.0, 24.0])
            .striped(true)
            .show(ui, |ui| {
                ui.label("Input");
                ui.add(DirectInput::new(
                    synth.clone(),
                    Input::Spectrum,
                    self.module_id,
                ));
                ui.end_row();

                ui.label("Gain");
                if ui
                    .add(ModulationInput::new(
                        &mut self.ui_state.gain,
                        synth.clone(),
                        Input::Gain,
                        self.module_id,
                    ))
                    .changed()
                {
                    bridge.set_param(Input::Gain, self.ui_state.gain);
                }
                ui.end_row();

                ui.label("Pitch shift");
                if ui
                    .add(ModulationInput::new(
                        &mut self.ui_state.pitch_shift,
                        synth.clone(),
                        Input::PitchShift,
                        self.module_id,
                    ))
                    .changed()
                {
                    bridge.set_param(Input::PitchShift, self.ui_state.pitch_shift);
                }
                ui.end_row();

                ui.label("Phase shift");
                if ui
                    .add(ModulationInput::new(
                        &mut self.ui_state.phase_shift,
                        synth.clone(),
                        Input::PhaseShift,
                        self.module_id,
                    ))
                    .changed()
                {
                    bridge.set_param(Input::PhaseShift, self.ui_state.phase_shift);
                }
                ui.end_row();

                ui.label("Frequency shift");
                if ui
                    .add(ModulationInput::new(
                        &mut self.ui_state.frequency_shift,
                        synth.clone(),
                        Input::FrequencyShift,
                        self.module_id,
                    ))
                    .changed()
                {
                    bridge.set_param(Input::FrequencyShift, self.ui_state.frequency_shift);
                }
                ui.end_row();

                ui.label("Detune");
                if ui
                    .add(ModulationInput::new(
                        &mut self.ui_state.detune,
                        synth.clone(),
                        Input::Detune,
                        self.module_id,
                    ))
                    .changed()
                {
                    bridge.set_param(Input::Detune, self.ui_state.detune);
                }
                ui.end_row();

                ui.label("Detune power");
                if ui
                    .add(ModulationInput::new(
                        &mut self.ui_state.detune_power,
                        synth.clone(),
                        Input::DetunePower,
                        self.module_id,
                    ))
                    .changed()
                {
                    bridge.set_param(Input::DetunePower, self.ui_state.detune_power);
                }
                ui.end_row();

                ui.label("Glide");
                if ui
                    .add(ModulationInput::new(
                        &mut self.ui_state.glide,
                        synth.clone(),
                        Input::Glide,
                        self.module_id,
                    ))
                    .changed()
                {
                    bridge.set_param(Input::Glide, self.ui_state.glide);
                }
                ui.end_row();

                ui.label("Glide Slope");
                if ui
                    .add(ModulationInput::new(
                        &mut self.ui_state.glide_slope,
                        synth.clone(),
                        Input::GlideSlope,
                        self.module_id,
                    ))
                    .changed()
                {
                    bridge.set_param(Input::GlideSlope, self.ui_state.glide_slope);
                }
                ui.end_row();

                ui.label("Steal phase");
                if ui
                    .add(Checkbox::without_text(&mut self.ui_state.steal_phase))
                    .changed()
                {
                    bridge.set_steal_phase(self.ui_state.steal_phase);
                }
                ui.end_row();

                ui.label("Unison");
                if ui
                    .add(DragValue::new(&mut self.ui_state.unison).range(1..=16))
                    .changed()
                {
                    bridge.set_unison(self.ui_state.unison);
                }
                ui.end_row();

                if self.ui_state.unison > 1 {
                    self.show_unison_section(&mut bridge, synth, ui);
                }
            });

        ui.add_space(40.0);

        if let Some(mut state) = self.gain_shape_state.take()
            && self.show_gain_shape_modal(&mut bridge, ui, &mut state)
        {
            self.gain_shape_state.replace(state);
        }

        if let Some(mut state) = self.randomize_phase_state.take()
            && self.show_randomize_phases_modal(&mut bridge, ui, &mut state)
        {
            self.randomize_phase_state.replace(state);
        }

        if confirm_module_removal(ui, &mut self.remove_confirmation) {
            synth.lock().remove_module(self.module_id);
        }

        self.bridge.replace(bridge);
    }

    fn cleanup(&mut self, synth: &SynthEngineHandle) {
        let Some(bridge) = self.bridge.take() else {
            return;
        };

        synth
            .lock()
            .get_typed_module_mut::<Oscillator>(self.module_id)
            .unwrap()
            .return_audio_bridge(bridge);
    }
}
