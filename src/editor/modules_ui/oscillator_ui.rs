use egui::{Checkbox, DragValue, Grid, Id, Modal, Sides, Ui};

use crate::{
    editor::{
        ModuleUi, db_slider::DbSlider, direct_input::DirectInput, gain_slider::GainSlider,
        modulation_input::ModulationInput, module_label::ModuleLabel, stereo_slider::StereoSlider,
        utils::confirm_module_removal,
    },
    synth_engine::{
        Input, ModuleId, Sample, StereoSample,
        oscillator::{self, OscillatorConfig, OscillatorUiBridge, PhasesDst},
        ui_bridge::{ModuleBridge, UiBridge},
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
    module_id: ModuleId,
    remove_confirmation: bool,
    label_state: Option<String>,
    unison_state: UnisonState,
}

impl OscillatorUI {
    pub fn new(module_id: ModuleId) -> Self {
        Self {
            module_id,
            remove_confirmation: false,
            label_state: None,
            unison_state: UnisonState {
                phases_shift_to: false,
                gains_to: false,
                gain_shape_state: None,
                randomize_phase_state: None,
            },
        }
    }

    fn show_gain_shape_modal(
        bridge: &mut oscillator::OscillatorUiBridge,
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
        bridge: &mut oscillator::OscillatorUiBridge,
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
        module_id: ModuleId,
        synth_bridge: &mut UiBridge,
        bridge: &mut oscillator::OscillatorUiBridge,
        config: &mut OscillatorConfig,
        unison_state: &mut UnisonState,
        ui: &mut Ui,
    ) {
        let unison = config.unison_voices;

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
                Self::show_phases(ui, (0..unison).map(|i| config.unison[i].initial_phase))
            {
                config.unison[voice_idx].initial_phase = phase;
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
                    if let Some((voice_idx, phase)) =
                        Self::show_phases(ui, (0..unison).map(|i| config.unison[i].phase_shift_to))
                    {
                        config.unison[voice_idx].phase_shift_to = phase;
                        bridge.set_unison_phase_shift_to(voice_idx, phase);
                    }
                } else if let Some((voice_idx, phase)) =
                    Self::show_phases(ui, (0..unison).map(|i| config.unison[i].phase_shift))
                {
                    config.unison[voice_idx].phase_shift = phase;
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
                &mut config.phases_blend,
                synth_bridge,
                Input::PhasesBlend,
                module_id,
            ))
            .changed()
        {
            bridge.set_param(Input::PhasesBlend, config.phases_blend);
        }
        ui.end_row();

        ui.label("Levels");
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                from_to_toggle(ui, &mut unison_state.gains_to);
            });

            if unison_state.gains_to {
                if let Some((voice_idx, gain)) =
                    Self::show_gains(ui, (0..unison).map(|i| config.unison[i].gain_to))
                {
                    config.unison[voice_idx].gain_to = gain;
                    bridge.set_unison_gain_to(voice_idx, gain);
                }
            } else if let Some((voice_idx, gain)) =
                Self::show_gains(ui, (0..unison).map(|i| config.unison[i].gain))
            {
                config.unison[voice_idx].gain = gain;
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
                &mut config.gains_blend,
                synth_bridge,
                Input::GainsBlend,
                module_id,
            ))
            .changed()
        {
            bridge.set_param(Input::GainsBlend, config.gains_blend);
        }
        ui.end_row();
    }

    fn paint_ui(
        &mut self,
        bridge: &mut UiBridge,
        osc_bridge: &mut OscillatorUiBridge,
        ui: &mut Ui,
    ) {
        let module_id = self.module_id;

        ui.add(ModuleLabel::new(&mut self.label_state, bridge, module_id));

        ui.add_space(20.0);

        let mut config = osc_bridge.config().clone();

        Grid::new("osc_grid")
            .num_columns(2)
            .spacing([40.0, 24.0])
            .striped(true)
            .show(ui, |ui| {
                ui.label("Input");
                ui.add(DirectInput::new(bridge, Input::Spectrum, module_id));
                ui.end_row();

                ui.label("Gain");
                if ui
                    .add(ModulationInput::new(
                        &mut config.gain,
                        bridge,
                        Input::Gain,
                        module_id,
                    ))
                    .changed()
                {
                    osc_bridge.set_param(Input::Gain, config.gain);
                }
                ui.end_row();

                ui.label("Pitch shift");
                if ui
                    .add(ModulationInput::new(
                        &mut config.pitch_shift,
                        bridge,
                        Input::PitchShift,
                        module_id,
                    ))
                    .changed()
                {
                    osc_bridge.set_param(Input::PitchShift, config.pitch_shift);
                }
                ui.end_row();

                ui.label("Phase shift");
                if ui
                    .add(ModulationInput::new(
                        &mut config.phase_shift,
                        bridge,
                        Input::PhaseShift,
                        module_id,
                    ))
                    .changed()
                {
                    osc_bridge.set_param(Input::PhaseShift, config.phase_shift);
                }
                ui.end_row();

                ui.label("Frequency shift");
                if ui
                    .add(ModulationInput::new(
                        &mut config.frequency_shift,
                        bridge,
                        Input::FrequencyShift,
                        module_id,
                    ))
                    .changed()
                {
                    osc_bridge.set_param(Input::FrequencyShift, config.frequency_shift);
                }
                ui.end_row();

                ui.label("Detune");
                if ui
                    .add(ModulationInput::new(
                        &mut config.detune,
                        bridge,
                        Input::Detune,
                        module_id,
                    ))
                    .changed()
                {
                    osc_bridge.set_param(Input::Detune, config.detune);
                }
                ui.end_row();

                ui.label("Detune power");
                if ui
                    .add(ModulationInput::new(
                        &mut config.detune_power,
                        bridge,
                        Input::DetunePower,
                        module_id,
                    ))
                    .changed()
                {
                    osc_bridge.set_param(Input::DetunePower, config.detune_power);
                }
                ui.end_row();

                ui.label("Glide");
                if ui
                    .add(ModulationInput::new(
                        &mut config.glide,
                        bridge,
                        Input::Glide,
                        module_id,
                    ))
                    .changed()
                {
                    osc_bridge.set_param(Input::Glide, config.glide);
                }
                ui.end_row();

                ui.label("Glide Slope");
                if ui
                    .add(ModulationInput::new(
                        &mut config.glide_slope,
                        bridge,
                        Input::GlideSlope,
                        module_id,
                    ))
                    .changed()
                {
                    osc_bridge.set_param(Input::GlideSlope, config.glide_slope);
                }
                ui.end_row();

                ui.label("Steal phase");
                if ui
                    .add(Checkbox::without_text(&mut config.steal_phase))
                    .changed()
                {
                    osc_bridge.set_steal_phase(config.steal_phase);
                }
                ui.end_row();

                ui.label("Unison");
                if ui
                    .add(DragValue::new(&mut config.unison_voices).range(1..=16))
                    .changed()
                {
                    osc_bridge.set_unison(config.unison_voices);
                }
                ui.end_row();

                if config.unison_voices > 1 {
                    Self::show_unison_section(
                        self.module_id,
                        bridge,
                        osc_bridge,
                        &mut config,
                        &mut self.unison_state,
                        ui,
                    );
                }
            });

        ui.add_space(40.0);

        if let Some(mut state) = self.unison_state.gain_shape_state.take()
            && Self::show_gain_shape_modal(osc_bridge, ui, &mut state)
        {
            self.unison_state.gain_shape_state.replace(state);
        }

        if let Some(mut state) = self.unison_state.randomize_phase_state.take()
            && Self::show_randomize_phases_modal(osc_bridge, ui, &mut state)
        {
            self.unison_state.randomize_phase_state.replace(state);
        }

        if confirm_module_removal(ui, &mut self.remove_confirmation) {
            bridge.remove_module(module_id);
        }
    }
}

impl ModuleUi for OscillatorUI {
    fn module_id(&self) -> Option<ModuleId> {
        Some(self.module_id)
    }

    fn ui(&mut self, bridge: &mut UiBridge, ui: &mut Ui) {
        bridge.with_module_bridge(self.module_id, |bridge, module_bridge| {
            if let ModuleBridge::Oscillator(osc_bridge) = module_bridge {
                self.paint_ui(bridge, osc_bridge, ui)
            }
        });
    }
}
