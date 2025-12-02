use egui_baseview::egui::{Checkbox, ComboBox, Grid, Slider, Ui};

use crate::{
    editor::{
        ModuleUI, modulation_input::ModulationInput, module_label::ModuleLabel,
        utils::confirm_module_removal,
    },
    synth_engine::{Envelope, EnvelopeCurve, Input, ModuleId, Sample, SynthEngine},
    utils::from_ms,
};

pub struct EnvelopeUI {
    module_id: ModuleId,
    remove_confirmation: bool,
    label_state: Option<String>,
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum DisplayCurve {
    Linear,
    PowerIn,
    PowerOut,
    ExponentialIn,
    ExponentialOut,
}

impl DisplayCurve {
    fn label(&self) -> &'static str {
        match self {
            Self::Linear => "Linear",
            Self::PowerIn => "Power In",
            Self::PowerOut => "Power Out",
            Self::ExponentialIn => "Exponential In",
            Self::ExponentialOut => "Exponential Out",
        }
    }

    fn env_curve(&self) -> EnvelopeCurve {
        match self {
            Self::Linear => EnvelopeCurve::Linear { full_range: true },
            Self::PowerIn => EnvelopeCurve::PowerIn {
                full_range: true,
                curvature: 0.2,
            },
            Self::PowerOut => EnvelopeCurve::PowerOut {
                full_range: true,
                curvature: 0.2,
            },
            Self::ExponentialIn => EnvelopeCurve::ExponentialIn { full_range: true },
            Self::ExponentialOut => EnvelopeCurve::ExponentialOut { full_range: true },
        }
    }
}

static CURVE_OPTIONS: &[DisplayCurve] = &[
    DisplayCurve::Linear,
    DisplayCurve::PowerIn,
    DisplayCurve::PowerOut,
    DisplayCurve::ExponentialIn,
    DisplayCurve::ExponentialOut,
];

impl EnvelopeCurve {
    fn display_curve(&self) -> DisplayCurve {
        match self {
            Self::Linear { .. } => DisplayCurve::Linear,
            Self::PowerIn { .. } => DisplayCurve::PowerIn,
            Self::PowerOut { .. } => DisplayCurve::PowerOut,
            Self::ExponentialIn { .. } => DisplayCurve::ExponentialIn,
            Self::ExponentialOut { .. } => DisplayCurve::ExponentialOut,
        }
    }
}

impl EnvelopeUI {
    pub fn new(module_id: ModuleId) -> Self {
        Self {
            module_id,
            remove_confirmation: false,
            label_state: None,
        }
    }

    fn env<'a>(&mut self, synth: &'a mut SynthEngine) -> &'a mut Envelope {
        Envelope::downcast_mut_unwrap(synth.get_module_mut(self.module_id))
    }

    fn add_curve(&self, ui: &mut Ui, label: &str, env_curve: &mut EnvelopeCurve) -> bool {
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
                changed = changed || ui.add(Slider::new(curvature, 0.0..=1.0)).changed();
            };

            match env_curve {
                EnvelopeCurve::PowerIn { curvature, .. } => {
                    add_curvature_slider(curvature);
                }
                EnvelopeCurve::PowerOut { curvature, .. } => {
                    add_curvature_slider(curvature);
                }
                _ => (),
            }

            let mut add_full_range_checkbox = |full_range: &mut bool| {
                changed = changed || ui.add(Checkbox::new(full_range, "Full range")).changed();
            };

            match env_curve {
                EnvelopeCurve::Linear { full_range, .. } => {
                    add_full_range_checkbox(full_range);
                }
                EnvelopeCurve::PowerIn { full_range, .. } => {
                    add_full_range_checkbox(full_range);
                }
                EnvelopeCurve::PowerOut { full_range, .. } => {
                    add_full_range_checkbox(full_range);
                }
                EnvelopeCurve::ExponentialIn { full_range, .. } => {
                    add_full_range_checkbox(full_range);
                }
                EnvelopeCurve::ExponentialOut { full_range, .. } => {
                    add_full_range_checkbox(full_range);
                }
            }
        });

        ui.end_row();

        changed
    }
}

impl ModuleUI for EnvelopeUI {
    fn module_id(&self) -> ModuleId {
        self.module_id
    }

    fn ui(&mut self, synth: &mut SynthEngine, ui: &mut Ui) {
        let id = self.module_id;
        let mut ui_data = self.env(synth).get_ui();

        ui.add(ModuleLabel::new(
            &ui_data.label,
            &mut self.label_state,
            synth.get_module_mut(self.module_id).unwrap(),
        ));

        ui.add_space(20.0);

        Grid::new("env_grid")
            .num_columns(2)
            .spacing([40.0, 24.0])
            .striped(true)
            .show(ui, |ui| {
                ui.label("Attack");
                if ui
                    .add(
                        ModulationInput::new(&mut ui_data.attack, synth, Input::Attack, id)
                            .default(from_ms(4.0)),
                    )
                    .changed()
                {
                    self.env(synth).set_attack(ui_data.attack);
                }
                ui.end_row();

                if self.add_curve(ui, "Attack Curve", &mut ui_data.attack_curve) {
                    self.env(synth).set_attack_curve(ui_data.attack_curve);
                }

                ui.label("Hold");
                if ui
                    .add(ModulationInput::new(
                        &mut ui_data.hold,
                        synth,
                        Input::Hold,
                        id,
                    ))
                    .changed()
                {
                    self.env(synth).set_hold(ui_data.hold);
                }
                ui.end_row();

                ui.label("Decay");
                if ui
                    .add(
                        ModulationInput::new(&mut ui_data.decay, synth, Input::Decay, id)
                            .default(from_ms(150.0)),
                    )
                    .changed()
                {
                    self.env(synth).set_decay(ui_data.decay);
                }
                ui.end_row();

                if self.add_curve(ui, "Decay Curve", &mut ui_data.decay_curve) {
                    self.env(synth).set_decay_curve(ui_data.decay_curve);
                }

                ui.label("Sustain");
                if ui
                    .add(ModulationInput::new(
                        &mut ui_data.sustain,
                        synth,
                        Input::Sustain,
                        id,
                    ))
                    .changed()
                {
                    self.env(synth).set_sustain(ui_data.sustain);
                }
                ui.end_row();

                ui.label("Release");
                if ui
                    .add(
                        ModulationInput::new(&mut ui_data.release, synth, Input::Release, id)
                            .default(from_ms(250.0)),
                    )
                    .changed()
                {
                    self.env(synth).set_release(ui_data.release);
                }
                ui.end_row();

                if self.add_curve(ui, "Release Curve", &mut ui_data.release_curve) {
                    self.env(synth).set_release_curve(ui_data.release_curve);
                }

                ui.label("Keep voice alive");
                if ui
                    .add(Checkbox::without_text(&mut ui_data.keep_voice_alive))
                    .changed()
                {
                    self.env(synth)
                        .set_keep_voice_alive(ui_data.keep_voice_alive);
                }
                ui.end_row();
            });

        ui.add_space(40.0);

        if confirm_module_removal(ui, &mut self.remove_confirmation) {
            synth.remove_module(self.module_id);
        }
    }
}
