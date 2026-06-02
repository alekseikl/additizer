use egui::{Checkbox, DragValue, Grid, Id, Modal, Sides, Ui};

use crate::{
    editor::{
        ModuleUi, db_slider::DbSlider, direct_input::DirectInput, gain_slider::GainSlider,
        modulation_input::ModulationInput, module_label::ModuleLabel, stereo_slider::StereoSlider,
        utils::confirm_module_removal,
    },
    synth_engine::{
        Input, ModuleId, Sample, StereoSample,
        oscillator::{self, ControlsState, PhasesDst},
        ui_bridge::UiBridge,
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

struct UnisonState {
    phases_shift_to: bool,
    gains_to: bool,
    gain_shape_state: Option<Box<GainShapeState>>,
    randomize_phase_state: Option<Box<RandomizePhaseState>>,
}

pub struct OscillatorUI {
    remove_confirmation: bool,
    label_state: Option<String>,
    osc_bridge: oscillator::UiBridge,
    unison_state: UnisonState,
}

impl OscillatorUI {
    pub fn new(module_id: ModuleId, synth_bridge: &mut UiBridge) -> Option<Self> {
        let osc_bridge = oscillator::UiBridge::create(module_id, synth_bridge.synth().clone())?;

        Some(Self {
            remove_confirmation: false,
            label_state: None,
            osc_bridge,
            unison_state: UnisonState {
                phases_shift_to: false,
                gains_to: false,
                gain_shape_state: None,
                randomize_phase_state: None,
            },
        })
    }

    fn show_gain_shape_modal(
        bridge: &mut oscillator::UiBridge,
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
        bridge: &mut oscillator::UiBridge,
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
        synth_bridge: &mut UiBridge,
        bridge: &mut oscillator::UiBridge,
        controls: &mut ControlsState,
        unison_state: &mut UnisonState,
        ui: &mut Ui,
    ) {
        let module_id = bridge.module_id();
        let unison = controls.unison;

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
                (0..unison).map(|i| controls.unison_params[i].initial_phase),
            ) {
                controls.unison_params[voice_idx].initial_phase = phase;
                bridge.set_unison_initial_phase(voice_idx, phase);
            }

            ui.add_space(8.0);

            if ui.button("Randomize").clicked() {
                unison_state.randomize_phase_state = Some(Box::new(RandomizePhaseState {
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
                from_to_toggle(ui, &mut unison_state.phases_shift_to);
            });

            ui.vertical(|ui| {
                if unison_state.phases_shift_to {
                    if let Some((voice_idx, phase)) = Self::show_phases(
                        ui,
                        (0..unison).map(|i| controls.unison_params[i].phase_shift_to),
                    ) {
                        controls.unison_params[voice_idx].phase_shift_to = phase;
                        bridge.set_unison_phase_shift_to(voice_idx, phase);
                    }
                } else if let Some((voice_idx, phase)) = Self::show_phases(
                    ui,
                    (0..unison).map(|i| controls.unison_params[i].phase_shift),
                ) {
                    controls.unison_params[voice_idx].phase_shift = phase;
                    bridge.set_unison_phase_shift(voice_idx, phase);
                }

                ui.add_space(8.0);

                if ui.button("Randomize").clicked() {
                    unison_state.randomize_phase_state = Some(Box::new(RandomizePhaseState {
                        from: 0.0,
                        to: 1.0,
                        stereo_spread: 0.1,
                        dst: if unison_state.phases_shift_to {
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
                &mut controls.phases_blend,
                synth_bridge,
                Input::PhasesBlend,
                module_id,
            ))
            .changed()
        {
            bridge.set_param(Input::PhasesBlend, controls.phases_blend);
        }
        ui.end_row();

        ui.label("Levels");
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                from_to_toggle(ui, &mut unison_state.gains_to);
            });

            if unison_state.gains_to {
                if let Some((voice_idx, gain)) =
                    Self::show_gains(ui, (0..unison).map(|i| controls.unison_params[i].gain_to))
                {
                    controls.unison_params[voice_idx].gain_to = gain;
                    bridge.set_unison_gain_to(voice_idx, gain);
                }
            } else if let Some((voice_idx, gain)) =
                Self::show_gains(ui, (0..unison).map(|i| controls.unison_params[i].gain))
            {
                controls.unison_params[voice_idx].gain = gain;
                bridge.set_unison_gain(voice_idx, gain);
            }

            ui.add_space(8.0);

            if ui.button("Shape").clicked() {
                unison_state.gain_shape_state = Some(Box::new(GainShapeState {
                    center: 0.5.into(),
                    level: (-24.0).into(),
                    to: unison_state.gains_to,
                }));
            }
        });
        ui.end_row();

        ui.label("Levels Blend");
        if ui
            .add(ModulationInput::new(
                &mut controls.gains_blend,
                synth_bridge,
                Input::GainsBlend,
                module_id,
            ))
            .changed()
        {
            bridge.set_param(Input::GainsBlend, controls.gains_blend);
        }
        ui.end_row();
    }
}

impl ModuleUi for OscillatorUI {
    fn module_id(&self) -> Option<ModuleId> {
        Some(self.osc_bridge.module_id())
    }

    fn ui(&mut self, synth_bridge: &mut UiBridge, ui: &mut Ui) {
        self.osc_bridge.update();

        let module_id = self.osc_bridge.module_id();

        ui.add(ModuleLabel::new(
            &mut self.label_state,
            synth_bridge,
            module_id,
        ));

        ui.add_space(20.0);

        let mut controls = self.osc_bridge.controls().clone();

        Grid::new("osc_grid")
            .num_columns(2)
            .spacing([40.0, 24.0])
            .striped(true)
            .show(ui, |ui| {
                ui.label("Input");
                ui.add(DirectInput::new(synth_bridge, Input::Spectrum, module_id));
                ui.end_row();

                ui.label("Gain");
                if ui
                    .add(ModulationInput::new(
                        &mut controls.gain,
                        synth_bridge,
                        Input::Gain,
                        module_id,
                    ))
                    .changed()
                {
                    self.osc_bridge.set_param(Input::Gain, controls.gain);
                }
                ui.end_row();

                ui.label("Pitch shift");
                if ui
                    .add(ModulationInput::new(
                        &mut controls.pitch_shift,
                        synth_bridge,
                        Input::PitchShift,
                        module_id,
                    ))
                    .changed()
                {
                    self.osc_bridge
                        .set_param(Input::PitchShift, controls.pitch_shift);
                }
                ui.end_row();

                ui.label("Phase shift");
                if ui
                    .add(ModulationInput::new(
                        &mut controls.phase_shift,
                        synth_bridge,
                        Input::PhaseShift,
                        module_id,
                    ))
                    .changed()
                {
                    self.osc_bridge
                        .set_param(Input::PhaseShift, controls.phase_shift);
                }
                ui.end_row();

                ui.label("Frequency shift");
                if ui
                    .add(ModulationInput::new(
                        &mut controls.frequency_shift,
                        synth_bridge,
                        Input::FrequencyShift,
                        module_id,
                    ))
                    .changed()
                {
                    self.osc_bridge
                        .set_param(Input::FrequencyShift, controls.frequency_shift);
                }
                ui.end_row();

                ui.label("Detune");
                if ui
                    .add(ModulationInput::new(
                        &mut controls.detune,
                        synth_bridge,
                        Input::Detune,
                        module_id,
                    ))
                    .changed()
                {
                    self.osc_bridge.set_param(Input::Detune, controls.detune);
                }
                ui.end_row();

                ui.label("Detune power");
                if ui
                    .add(ModulationInput::new(
                        &mut controls.detune_power,
                        synth_bridge,
                        Input::DetunePower,
                        module_id,
                    ))
                    .changed()
                {
                    self.osc_bridge
                        .set_param(Input::DetunePower, controls.detune_power);
                }
                ui.end_row();

                ui.label("Glide");
                if ui
                    .add(ModulationInput::new(
                        &mut controls.glide,
                        synth_bridge,
                        Input::Glide,
                        module_id,
                    ))
                    .changed()
                {
                    self.osc_bridge.set_param(Input::Glide, controls.glide);
                }
                ui.end_row();

                ui.label("Glide Slope");
                if ui
                    .add(ModulationInput::new(
                        &mut controls.glide_slope,
                        synth_bridge,
                        Input::GlideSlope,
                        module_id,
                    ))
                    .changed()
                {
                    self.osc_bridge
                        .set_param(Input::GlideSlope, controls.glide_slope);
                }
                ui.end_row();

                ui.label("Steal phase");
                if ui
                    .add(Checkbox::without_text(&mut controls.steal_phase))
                    .changed()
                {
                    self.osc_bridge.set_steal_phase(controls.steal_phase);
                }
                ui.end_row();

                ui.label("Unison");
                if ui
                    .add(DragValue::new(&mut controls.unison).range(1..=16))
                    .changed()
                {
                    self.osc_bridge.set_unison(controls.unison);
                }
                ui.end_row();

                if controls.unison > 1 {
                    Self::show_unison_section(
                        synth_bridge,
                        &mut self.osc_bridge,
                        &mut controls,
                        &mut self.unison_state,
                        ui,
                    );
                }
            });

        ui.add_space(40.0);

        if let Some(mut state) = self.unison_state.gain_shape_state.take()
            && Self::show_gain_shape_modal(&mut self.osc_bridge, ui, &mut state)
        {
            self.unison_state.gain_shape_state.replace(state);
        }

        if let Some(mut state) = self.unison_state.randomize_phase_state.take()
            && Self::show_randomize_phases_modal(&mut self.osc_bridge, ui, &mut state)
        {
            self.unison_state.randomize_phase_state.replace(state);
        }

        if confirm_module_removal(ui, &mut self.remove_confirmation) {
            synth_bridge.remove_module(module_id);
        }
    }
}
