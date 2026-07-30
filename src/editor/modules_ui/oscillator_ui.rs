use egui::{Align, Checkbox, DragValue, Grid, Id, Layout, Modal, Sides, Ui};
use nih_plug::util::{db_to_gain, gain_to_db};

use crate::{
    editor::{
        ModuleUi,
        module_label::ModuleLabel,
        slider::{self, Slider},
        stereo_input::StereoInput,
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
    gain_shape_state: Option<Box<GainShapeState>>,
    randomize_phase_state: Option<Box<RandomizePhaseState>>,
}

pub struct OscillatorUI {
    module_id: ModuleId,
    label_state: Option<String>,
    unison_state: UnisonState,
}

impl OscillatorUI {
    pub fn new(module_id: ModuleId) -> Self {
        Self {
            module_id,
            label_state: None,
            unison_state: UnisonState {
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
                    ui.add(Slider::stereo(&mut state.center, 0.0..=1.0, None).default(0.5));
                    ui.end_row();

                    ui.label("Level");
                    ui.add(
                        Slider::stereo(&mut state.level, -48.0..=6.0, None)
                            .over(0.0)
                            .units(slider::Units::Db)
                            .default(0.0),
                    );
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

            Grid::new("randomize_phases-grid")
                .num_columns(2)
                .spacing([40.0, 24.0])
                .striped(true)
                .show(ui, |ui| {
                    ui.label("From");
                    ui.add(Slider::mono(&mut state.from, 0.0..=1.0, None).default(0.0));
                    ui.end_row();

                    ui.label("To");
                    ui.add(Slider::mono(&mut state.to, 0.0..=1.0, None).default(1.0));
                    ui.end_row();

                    ui.label("Stereo spread");
                    ui.add(Slider::mono(&mut state.stereo_spread, 0.0..=1.0, None).default(1.0));
                    ui.end_row();
                });

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

        ui.horizontal(|ui| {
            for (voice_idx, mut phase) in phases.enumerate() {
                if ui
                    .add(
                        Slider::stereo(&mut phase, 0.0..=1.0, None)
                            .vertical()
                            .thickness(12.0)
                            .length(100.0)
                            .default(0.0),
                    )
                    .changed()
                {
                    result = Some((voice_idx, phase));
                }
            }
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
                for (voice_idx, gain) in gains.enumerate() {
                    let mut gain_db = gain.map(gain_to_db);

                    if ui
                        .add(
                            Slider::stereo(&mut gain_db, -48.0..=6.0, None)
                                .over(0.0)
                                .units(slider::Units::Db)
                                .vertical()
                                .thickness(12.0)
                                .length(100.0)
                                .skew(2.0)
                                .default(0.0),
                        )
                        .changed()
                    {
                        result = Some((voice_idx, gain_db.map(db_to_gain)));
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
                ui.vertical(|ui| {
                    if let Some((voice_idx, phase)) =
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
                            dst: PhasesDst::From,
                        }));
                    }
                });

                ui.vertical(|ui| {
                    ui.add_space(40.0);
                    ui.label("->");
                });

                ui.vertical(|ui| {
                    if let Some((voice_idx, phase)) =
                        Self::show_phases(ui, (0..unison).map(|i| config.unison[i].phase_shift_to))
                    {
                        config.unison[voice_idx].phase_shift_to = phase;
                        bridge.set_unison_phase_shift_to(voice_idx, phase);
                    }

                    ui.add_space(8.0);

                    if ui.button("Randomize").clicked() {
                        unison_state.randomize_phase_state = Some(Box::new(RandomizePhaseState {
                            from: 0.0,
                            to: 1.0,
                            stereo_spread: 0.1,
                            dst: PhasesDst::To,
                        }));
                    }
                });
            });
        });
        ui.end_row();

        ui.label("Phases Blend");
        if ui
            .add(StereoInput::new(
                Input::PhasesBlend,
                module_id,
                &mut config.phases_blend,
                synth_bridge,
            ))
            .changed()
        {
            bridge.set_param(Input::PhasesBlend, config.phases_blend);
        }
        ui.end_row();

        ui.label("Levels");
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    if let Some((voice_idx, gain)) =
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
                            to: false,
                        }));
                    }
                });

                ui.vertical(|ui| {
                    ui.add_space(40.0);
                    ui.label("->");
                });

                ui.vertical(|ui| {
                    if let Some((voice_idx, gain)) =
                        Self::show_gains(ui, (0..unison).map(|i| config.unison[i].gain_to))
                    {
                        config.unison[voice_idx].gain_to = gain;
                        bridge.set_unison_gain_to(voice_idx, gain);
                    }

                    ui.add_space(8.0);

                    if ui.button("Shape").clicked() {
                        unison_state.gain_shape_state = Some(Box::new(GainShapeState {
                            center: 0.5.into(),
                            level: (-24.0).into(),
                            to: true,
                        }));
                    }
                });

                ui.add_space(8.0);
            });
        });
        ui.end_row();

        ui.label("Levels Blend");
        if ui
            .add(StereoInput::new(
                Input::GainsBlend,
                module_id,
                &mut config.gains_blend,
                synth_bridge,
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

        let label = |ui: &mut Ui, text: &str| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(text);
            });
        };

        Grid::new("osc_grid")
            .num_columns(4)
            .spacing([8.0, 8.0])
            .show(ui, |ui| {
                label(ui, "Gain");

                ui.horizontal(|ui| {
                    if ui
                        .add(StereoInput::new(
                            Input::Gain,
                            module_id,
                            &mut config.gain,
                            bridge,
                        ))
                        .changed()
                    {
                        osc_bridge.set_param(Input::Gain, config.gain);
                    }

                    ui.add_space(8.0);
                });

                label(ui, "Pitch shift");
                if ui
                    .add(StereoInput::new(
                        Input::PitchShift,
                        module_id,
                        &mut config.pitch_shift,
                        bridge,
                    ))
                    .changed()
                {
                    osc_bridge.set_param(Input::PitchShift, config.pitch_shift);
                }
                ui.end_row();

                label(ui, "Phase shift");
                if ui
                    .add(StereoInput::new(
                        Input::PhaseShift,
                        module_id,
                        &mut config.phase_shift,
                        bridge,
                    ))
                    .changed()
                {
                    osc_bridge.set_param(Input::PhaseShift, config.phase_shift);
                }

                label(ui, "Frequency shift");
                if ui
                    .add(StereoInput::new(
                        Input::FrequencyShift,
                        module_id,
                        &mut config.frequency_shift,
                        bridge,
                    ))
                    .changed()
                {
                    osc_bridge.set_param(Input::FrequencyShift, config.frequency_shift);
                }
                ui.end_row();

                label(ui, "Detune");
                if ui
                    .add(StereoInput::new(
                        Input::Detune,
                        module_id,
                        &mut config.detune,
                        bridge,
                    ))
                    .changed()
                {
                    osc_bridge.set_param(Input::Detune, config.detune);
                }

                label(ui, "Detune power");
                if ui
                    .add(StereoInput::new(
                        Input::DetunePower,
                        module_id,
                        &mut config.detune_power,
                        bridge,
                    ))
                    .changed()
                {
                    osc_bridge.set_param(Input::DetunePower, config.detune_power);
                }
                ui.end_row();

                label(ui, "Glide");
                if ui
                    .add(StereoInput::new(
                        Input::Glide,
                        module_id,
                        &mut config.glide,
                        bridge,
                    ))
                    .changed()
                {
                    osc_bridge.set_param(Input::Glide, config.glide);
                }

                label(ui, "Glide Slope");
                if ui
                    .add(StereoInput::new(
                        Input::GlideSlope,
                        module_id,
                        &mut config.glide_slope,
                        bridge,
                    ))
                    .changed()
                {
                    osc_bridge.set_param(Input::GlideSlope, config.glide_slope);
                }
                ui.end_row();

                label(ui, "Unison");
                if ui
                    .add(DragValue::new(&mut config.unison_voices).range(1..=16))
                    .changed()
                {
                    osc_bridge.set_unison(config.unison_voices);
                }

                label(ui, "Steal phase");
                if ui
                    .add(Checkbox::without_text(&mut config.steal_phase))
                    .changed()
                {
                    osc_bridge.set_steal_phase(config.steal_phase);
                }
                ui.end_row();
            });

        if config.unison_voices > 1 {
            ui.add_space(32.0);

            Grid::new("osc_unison_grid")
                .num_columns(2)
                .spacing([24.0, 24.0])
                .striped(true)
                .show(ui, |ui| {
                    Self::show_unison_section(
                        self.module_id,
                        bridge,
                        osc_bridge,
                        &mut config,
                        &mut self.unison_state,
                        ui,
                    );
                });
        }

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
