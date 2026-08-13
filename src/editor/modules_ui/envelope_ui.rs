use egui::{Checkbox, Grid, Ui};

use crate::{
    editor::{ModuleUi, module_label::ModuleLabel, slider::Slider, stereo_input::StereoInput},
    synth_engine::{
        Input, ModuleId, ModuleType,
        envelope::EnvelopeUiBridge,
        ui_bridge::{ModuleBridge, UiBridge},
    },
};

pub struct EnvelopeUI {
    module_id: ModuleId,
}

impl EnvelopeUI {
    pub fn new(module_id: ModuleId) -> Self {
        Self { module_id }
    }

    fn paint_ui(&mut self, bridge: &mut UiBridge, env_bridge: &mut EnvelopeUiBridge, ui: &mut Ui) {
        let module_id = self.module_id;
        let mut config = env_bridge.config().clone();

        ui.add(ModuleLabel::new(module_id, ModuleType::Envelope, bridge));

        ui.add_space(16.0);

        Grid::new("env_grid")
            .num_columns(4)
            .spacing([8.0, 8.0])
            .show(ui, |ui| {
                ui.label("Delay");
                ui.horizontal(|ui| {
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

                    ui.add_space(8.0);
                });

                ui.label("Hold");
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

                ui.label("Attack");
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

                ui.label("Attack Curve");
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

                ui.label("Decay");
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

                ui.label("Decay Curve");
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

                ui.label("Sustain");
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
                ui.end_row();

                ui.label("Release");
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

                ui.label("Release Curve");
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

                ui.label("Keep alive").on_hover_text("Keeps voice alive");
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
