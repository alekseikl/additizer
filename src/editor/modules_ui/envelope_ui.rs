use egui::{Checkbox, Grid, Slider, Ui};

use crate::{
    editor::{
        ModuleUi, modulation_input::ModulationInput, module_label::ModuleLabel,
        stereo_slider::StereoSlider, utils::confirm_module_removal,
    },
    synth_engine::{
        Input, ModuleId,
        envelope::EnvelopeUiBridge,
        ui_bridge::{ModuleBridge, UiBridge},
    },
    utils::from_ms,
};

pub struct EnvelopeUI {
    module_id: ModuleId,
    remove_confirmation: bool,
    label_state: Option<String>,
}

impl EnvelopeUI {
    pub fn new(module_id: ModuleId) -> Self {
        Self {
            module_id,
            remove_confirmation: false,
            label_state: None,
        }
    }

    fn paint_ui(
        &mut self,
        bridge: &mut UiBridge,
        env_bridge: &mut EnvelopeUiBridge,
        ui: &mut Ui,
    ) {
        let module_id = self.module_id;
        let mut config = env_bridge.config().clone();

        ui.add(ModuleLabel::new(
            &mut self.label_state,
            bridge,
            module_id,
        ));

        ui.add_space(20.0);

        Grid::new("env_grid")
            .num_columns(2)
            .spacing([40.0, 24.0])
            .striped(true)
            .show(ui, |ui| {
                ui.label("Delay");
                if ui
                    .add(
                        ModulationInput::new(&mut config.delay, bridge, Input::Delay, module_id)
                            .default(from_ms(0.0)),
                    )
                    .changed()
                {
                    env_bridge.set_param(Input::Delay, config.delay);
                }
                ui.end_row();

                ui.label("Attack");
                if ui
                    .add(
                        ModulationInput::new(
                            &mut config.attack,
                            bridge,
                            Input::Attack,
                            module_id,
                        )
                        .default(from_ms(0.0)),
                    )
                    .changed()
                {
                    env_bridge.set_param(Input::Attack, config.attack);
                }
                ui.end_row();

                ui.label("Attack Curve");
                if ui
                    .add(Slider::new(&mut config.attack_curvature, -1.0..=1.0))
                    .changed()
                {
                    env_bridge.set_attack_curvature(config.attack_curvature);
                }
                ui.end_row();

                ui.label("Hold");
                if ui
                    .add(ModulationInput::new(
                        &mut config.hold,
                        bridge,
                        Input::Hold,
                        module_id,
                    ))
                    .changed()
                {
                    env_bridge.set_param(Input::Hold, config.hold);
                }
                ui.end_row();

                ui.label("Decay");
                if ui
                    .add(
                        ModulationInput::new(&mut config.decay, bridge, Input::Decay, module_id)
                            .default(from_ms(150.0)),
                    )
                    .changed()
                {
                    env_bridge.set_param(Input::Decay, config.decay);
                }
                ui.end_row();

                ui.label("Decay Curve");
                if ui
                    .add(Slider::new(&mut config.decay_curvature, -1.0..=1.0))
                    .changed()
                {
                    env_bridge.set_decay_curvature(config.decay_curvature);
                }
                ui.end_row();

                ui.label("Sustain");
                if ui
                    .add(ModulationInput::new(
                        &mut config.sustain,
                        bridge,
                        Input::Sustain,
                        module_id,
                    ))
                    .changed()
                {
                    env_bridge.set_param(Input::Sustain, config.sustain);
                }
                ui.end_row();

                ui.label("Release");
                if ui
                    .add(
                        ModulationInput::new(
                            &mut config.release,
                            bridge,
                            Input::Release,
                            module_id,
                        )
                        .default(from_ms(250.0)),
                    )
                    .changed()
                {
                    env_bridge.set_param(Input::Release, config.release);
                }
                ui.end_row();

                ui.label("Release Curve");
                if ui
                    .add(Slider::new(&mut config.release_curvature, -1.0..=1.0))
                    .changed()
                {
                    env_bridge.set_release_curvature(config.release_curvature);
                }
                ui.end_row();

                ui.label("Smooth");
                if ui
                    .add(
                        StereoSlider::new(&mut config.smooth)
                            .range(0.0..=0.1)
                            .display_scale(1000.0)
                            .default_value(0.0)
                            .skew(1.2)
                            .precision(1)
                            .units(" ms"),
                    )
                    .changed()
                {
                    env_bridge.set_smooth(config.smooth);
                }
                ui.end_row();

                ui.label("Keep voice alive");
                if ui
                    .add(Checkbox::without_text(&mut config.keep_voice_alive))
                    .changed()
                {
                    env_bridge.set_keep_voice_alive(config.keep_voice_alive);
                }
                ui.end_row();
            });

        ui.add_space(40.0);

        if confirm_module_removal(ui, &mut self.remove_confirmation) {
            bridge.remove_module(module_id);
        }
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
