use egui_baseview::egui::{Checkbox, ComboBox, Grid, Response, Slider, Ui, Widget};

use crate::{
    editor::modulation_input::ModulationInput,
    synth_engine::{Envelope, EnvelopeCurve, ModuleId, ModuleInput, Sample, SynthEngine},
    utils::from_ms,
};

pub struct EnvelopeUI<'a> {
    module_id: ModuleId,
    synth_engine: &'a mut SynthEngine,
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

    fn env_curve(&self, curvature: Sample) -> EnvelopeCurve {
        match self {
            Self::Linear => EnvelopeCurve::Linear,
            Self::PowerIn => EnvelopeCurve::PowerIn(curvature),
            Self::PowerOut => EnvelopeCurve::PowerOut(curvature),
            Self::ExponentialIn => EnvelopeCurve::ExponentialIn(curvature),
            Self::ExponentialOut => EnvelopeCurve::ExponentialOut(curvature),
        }
    }
}

const CURVE_OPTIONS: &[DisplayCurve] = &[
    DisplayCurve::Linear,
    DisplayCurve::PowerIn,
    DisplayCurve::PowerOut,
    DisplayCurve::ExponentialIn,
    DisplayCurve::ExponentialOut,
];

impl EnvelopeCurve {
    fn curvature(&self) -> Sample {
        match *self {
            Self::Linear => 0.0,
            Self::PowerIn(c) => c,
            Self::PowerOut(c) => c,
            Self::ExponentialIn(c) => c,
            Self::ExponentialOut(c) => c,
        }
    }

    fn display_curve(&self) -> DisplayCurve {
        match self {
            Self::Linear => DisplayCurve::Linear,
            Self::PowerIn(_) => DisplayCurve::PowerIn,
            Self::PowerOut(_) => DisplayCurve::PowerOut,
            Self::ExponentialIn(_) => DisplayCurve::ExponentialIn,
            Self::ExponentialOut(_) => DisplayCurve::ExponentialOut,
        }
    }
}

impl<'a> EnvelopeUI<'a> {
    pub fn new(module_id: ModuleId, synth_engine: &'a mut SynthEngine) -> Self {
        Self {
            module_id,
            synth_engine,
        }
    }

    fn env(&mut self) -> &mut Envelope {
        Envelope::downcast_mut_unwrap(self.synth_engine.get_module_mut(self.module_id))
    }

    fn add_curve(&self, ui: &mut Ui, label: &str, env_curve: &mut EnvelopeCurve) -> bool {
        let mut display_curve = env_curve.display_curve();
        let mut curvature = env_curve.curvature();
        let mut changed = false;

        ui.label(label);

        ui.horizontal(|ui| {
            ComboBox::from_id_salt(format!("curve-select-{}", label))
                .selected_text(display_curve.label())
                .show_ui(ui, |ui| {
                    for curve in CURVE_OPTIONS {
                        if ui
                            .selectable_label(*curve == display_curve, curve.label())
                            .clicked()
                        {
                            display_curve = *curve;
                            changed = true;
                        }
                    }
                });

            if display_curve != DisplayCurve::Linear {
                changed = changed || ui.add(Slider::new(&mut curvature, 0.0..=1.0)).changed();
            }
        });

        ui.end_row();

        if changed {
            *env_curve = display_curve.env_curve(curvature);
        }

        changed
    }
}

impl Widget for EnvelopeUI<'_> {
    fn ui(mut self, ui: &mut Ui) -> Response {
        let id = self.module_id;
        let mut ui_data = self.env().get_ui();

        ui.heading("Envelope");
        ui.add_space(20.0);

        Grid::new("env_grid")
            .num_columns(2)
            .spacing([40.0, 24.0])
            .striped(true)
            .show(ui, |ui| {
                ui.label("Attack");
                if ui
                    .add(
                        ModulationInput::new(
                            &mut ui_data.attack,
                            self.synth_engine,
                            ModuleInput::attack(id),
                        )
                        .default(from_ms(4.0)),
                    )
                    .changed()
                {
                    self.env().set_attack(ui_data.attack);
                }
                ui.end_row();

                if self.add_curve(ui, "Attack Curve", &mut ui_data.attack_curve) {
                    self.env().set_attack_curve(ui_data.attack_curve);
                }

                ui.label("Decay");
                if ui
                    .add(
                        ModulationInput::new(
                            &mut ui_data.decay,
                            self.synth_engine,
                            ModuleInput::decay(id),
                        )
                        .default(from_ms(150.0)),
                    )
                    .changed()
                {
                    self.env().set_decay(ui_data.decay);
                }
                ui.end_row();

                if self.add_curve(ui, "Decay Curve", &mut ui_data.decay_curve) {
                    self.env().set_decay_curve(ui_data.decay_curve);
                }

                ui.label("Sustain");
                if ui
                    .add(ModulationInput::new(
                        &mut ui_data.sustain,
                        self.synth_engine,
                        ModuleInput::sustain(id),
                    ))
                    .changed()
                {
                    self.env().set_sustain(ui_data.sustain);
                }
                ui.end_row();

                ui.label("Release");
                if ui
                    .add(
                        ModulationInput::new(
                            &mut ui_data.release,
                            self.synth_engine,
                            ModuleInput::release(id),
                        )
                        .default(from_ms(250.0)),
                    )
                    .changed()
                {
                    self.env().set_release(ui_data.release);
                }
                ui.end_row();

                if self.add_curve(ui, "Release Curve", &mut ui_data.release_curve) {
                    self.env().set_release_curve(ui_data.release_curve);
                }

                ui.label("Keep voice alive");
                if ui
                    .add(Checkbox::without_text(&mut ui_data.keep_voice_alive))
                    .changed()
                {
                    self.env().set_keep_voice_alive(ui_data.keep_voice_alive);
                }
                ui.end_row();
            })
            .response
    }
}
