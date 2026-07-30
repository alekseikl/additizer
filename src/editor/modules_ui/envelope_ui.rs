use egui::{Align, Checkbox, Grid, Layout, Ui};

use crate::{
    editor::{
        ModuleUi,
        module_label::ModuleLabel,
        slider::{self, Slider},
        stereo_input::StereoInput,
    },
    synth_engine::{
        Input, ModuleId,
        envelope::EnvelopeUiBridge,
        ui_bridge::{ModuleBridge, UiBridge},
    },
};

pub struct EnvelopeUI {
    module_id: ModuleId,
    label_state: Option<String>,
}

impl EnvelopeUI {
    pub fn new(module_id: ModuleId) -> Self {
        Self {
            module_id,
            label_state: None,
        }
    }

    fn paint_ui(&mut self, bridge: &mut UiBridge, env_bridge: &mut EnvelopeUiBridge, ui: &mut Ui) {
        let module_id = self.module_id;
        let mut config = env_bridge.config().clone();

        ui.add(ModuleLabel::new(&mut self.label_state, bridge, module_id));

        ui.add_space(20.0);

        let label = |ui: &mut Ui, text: &str| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(text);
            });
        };

        Grid::new("env_grid")
            .num_columns(4)
            .spacing([8.0, 8.0])
            .show(ui, |ui| {
                label(ui, "Delay");
                if ui
                    .add(StereoInput::new(
                        Input::Delay,
                        module_id,
                        &mut config.delay,
                        bridge,
                    ))
                    .changed()
                {
                    env_bridge.set_param(Input::Delay, config.delay);
                }

                label(ui, "Hold");
                if ui
                    .add(StereoInput::new(
                        Input::Hold,
                        module_id,
                        &mut config.hold,
                        bridge,
                    ))
                    .changed()
                {
                    env_bridge.set_param(Input::Hold, config.hold);
                }
                ui.end_row();

                label(ui, "Attack");
                if ui
                    .add(StereoInput::new(
                        Input::Attack,
                        module_id,
                        &mut config.attack,
                        bridge,
                    ))
                    .changed()
                {
                    env_bridge.set_param(Input::Attack, config.attack);
                }

                label(ui, "Attack Curve");
                if ui
                    .add(Slider::mono(
                        &mut config.attack_curvature,
                        0.0..=1.0,
                        Some(-1.0),
                    ))
                    .changed()
                {
                    env_bridge.set_attack_curvature(config.attack_curvature);
                }
                ui.end_row();

                label(ui, "Decay");
                if ui
                    .add(StereoInput::new(
                        Input::Decay,
                        module_id,
                        &mut config.decay,
                        bridge,
                    ))
                    .changed()
                {
                    env_bridge.set_param(Input::Decay, config.decay);
                }

                label(ui, "Decay Curve");
                if ui
                    .add(Slider::mono(
                        &mut config.decay_curvature,
                        0.0..=1.0,
                        Some(-1.0),
                    ))
                    .changed()
                {
                    env_bridge.set_decay_curvature(config.decay_curvature);
                }
                ui.end_row();

                label(ui, "Sustain");
                if ui
                    .add(StereoInput::new(
                        Input::Sustain,
                        module_id,
                        &mut config.sustain,
                        bridge,
                    ))
                    .changed()
                {
                    env_bridge.set_param(Input::Sustain, config.sustain);
                }

                label(ui, "Smooth");
                if ui
                    .add(
                        Slider::stereo(&mut config.smooth, 0.0..=0.1, None)
                            .default(0.0)
                            .skew(1.2)
                            .units(slider::Units::Time),
                    )
                    .changed()
                {
                    env_bridge.set_smooth(config.smooth);
                }
                ui.end_row();

                label(ui, "Release");
                if ui
                    .add(StereoInput::new(
                        Input::Release,
                        module_id,
                        &mut config.release,
                        bridge,
                    ))
                    .changed()
                {
                    env_bridge.set_param(Input::Release, config.release);
                }

                label(ui, "Release Curve");
                if ui
                    .add(Slider::mono(
                        &mut config.release_curvature,
                        0.0..=1.0,
                        Some(-1.0),
                    ))
                    .changed()
                {
                    env_bridge.set_release_curvature(config.release_curvature);
                }
                ui.end_row();

                label(ui, "Keep voice alive");
                if ui
                    .add(Checkbox::without_text(&mut config.keep_voice_alive))
                    .changed()
                {
                    env_bridge.set_keep_voice_alive(config.keep_voice_alive);
                }
                ui.end_row();
            });
    }
}

impl ModuleUi for EnvelopeUI {
    fn module_id(&self) -> Option<ModuleId> {
        Some(self.module_id)
    }

    fn ui(&mut self, bridge: &mut UiBridge, ui: &mut Ui) {
        bridge.with_module_bridge(self.module_id, |bridge, module_bridge| {
            if let ModuleBridge::Envelope(env_bridge) = module_bridge {
                self.paint_ui(bridge, env_bridge, ui);
            }
        });
    }
}
