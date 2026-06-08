use egui::{Checkbox, ComboBox, Grid, Slider, Ui};

use crate::{
    editor::{
        ModuleUi, modulation_input::ModulationInput, module_label::ModuleLabel,
        stereo_slider::StereoSlider, utils::confirm_module_removal,
    },
    synth_engine::{
        EnvelopeCurve, Input, ModuleId, Sample, envelope, ui_bridge::UiBridge,
    },
    utils::from_ms,
};

pub struct EnvelopeUI {
    remove_confirmation: bool,
    label_state: Option<String>,
    env_bridge: envelope::UiBridge,
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum DisplayCurve {
    Linear,
    Exponential,
    ExponentialIn,
    ExponentialOut,
}

impl DisplayCurve {
    fn label(&self) -> &'static str {
        match self {
            Self::Linear => "Linear",
            Self::Exponential => "Exponential",
            Self::ExponentialIn => "Exponential In",
            Self::ExponentialOut => "Exponential Out",
        }
    }

    fn env_curve(&self) -> EnvelopeCurve {
        match self {
            Self::Linear => EnvelopeCurve::Linear,
            Self::Exponential => EnvelopeCurve::Exponential { curvature: 0.5 },
            Self::ExponentialIn => EnvelopeCurve::ExponentialIn,
            Self::ExponentialOut => EnvelopeCurve::ExponentialOut,
        }
    }
}

static CURVE_OPTIONS: &[DisplayCurve] = &[
    DisplayCurve::Linear,
    DisplayCurve::Exponential,
    DisplayCurve::ExponentialIn,
    DisplayCurve::ExponentialOut,
];

impl EnvelopeCurve {
    fn display_curve(&self) -> DisplayCurve {
        match self {
            Self::Linear { .. } => DisplayCurve::Linear,
            Self::Exponential { .. } => DisplayCurve::Exponential,
            Self::ExponentialIn { .. } => DisplayCurve::ExponentialIn,
            Self::ExponentialOut { .. } => DisplayCurve::ExponentialOut,
        }
    }
}

impl EnvelopeUI {
    pub fn new(module_id: ModuleId, synth_bridge: &mut UiBridge) -> Option<Self> {
        let env_bridge = envelope::UiBridge::create(module_id, synth_bridge.engine().clone())?;

        Some(Self {
            remove_confirmation: false,
            label_state: None,
            env_bridge,
        })
    }

    fn add_curve(
        bridge: &mut envelope::UiBridge,
        ui: &mut Ui,
        label: &str,
        env_curve: &mut EnvelopeCurve,
        set_curve: impl FnOnce(&mut envelope::UiBridge, EnvelopeCurve),
    ) -> bool {
        let mut changed = false;

        ui.label(label);

        ui.horizontal(|ui| {
            let display_curve = env_curve.display_curve();

            ComboBox::from_id_salt(format!("curve-select-{}", label))
                .selected_text(display_curve.label())
                .show_ui(ui, |ui| {
                    for curve in CURVE_OPTIONS {
                        if ui
                            .selectable_label(*curve == display_curve, curve.label())
                            .clicked()
                        {
                            *env_curve = curve.env_curve();
                            changed = true;
                        }
                    }
                });

            let mut add_curvature_slider = |curvature: &mut Sample| {
                changed = changed || ui.add(Slider::new(curvature, -1.0..=1.0)).changed();
            };

            if let EnvelopeCurve::Exponential { curvature } = env_curve {
                add_curvature_slider(curvature);
            }
        });

        ui.end_row();

        if changed {
            set_curve(bridge, *env_curve);
        }

        changed
    }
}

impl ModuleUi for EnvelopeUI {
    fn module_id(&self) -> Option<ModuleId> {
        Some(self.env_bridge.module_id())
    }

    fn ui(&mut self, bridge: &mut UiBridge, ui: &mut Ui) {
        let module_id = self.env_bridge.module_id();
        let mut config = self.env_bridge.config().clone();

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
                    self.env_bridge.set_param(Input::Delay, config.delay);
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
                    self.env_bridge.set_param(Input::Attack, config.attack);
                }
                ui.end_row();

                Self::add_curve(
                    &mut self.env_bridge,
                    ui,
                    "Attack Curve",
                    &mut config.attack_curve,
                    envelope::UiBridge::set_attack_curve,
                );

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
                    self.env_bridge.set_param(Input::Hold, config.hold);
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
                    self.env_bridge.set_param(Input::Decay, config.decay);
                }
                ui.end_row();

                Self::add_curve(
                    &mut self.env_bridge,
                    ui,
                    "Decay Curve",
                    &mut config.decay_curve,
                    envelope::UiBridge::set_decay_curve,
                );

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
                    self.env_bridge.set_param(Input::Sustain, config.sustain);
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
                    self.env_bridge.set_param(Input::Release, config.release);
                }
                ui.end_row();

                Self::add_curve(
                    &mut self.env_bridge,
                    ui,
                    "Release Curve",
                    &mut config.release_curve,
                    envelope::UiBridge::set_release_curve,
                );

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
                    self.env_bridge.set_smooth(config.smooth);
                }
                ui.end_row();

                ui.label("Keep voice alive");
                if ui
                    .add(Checkbox::without_text(&mut config.keep_voice_alive))
                    .changed()
                {
                    self.env_bridge
                        .set_keep_voice_alive(config.keep_voice_alive);
                }
                ui.end_row();
            });

        ui.add_space(40.0);

        if confirm_module_removal(ui, &mut self.remove_confirmation) {
            bridge.remove_module(module_id);
        }
    }
}
