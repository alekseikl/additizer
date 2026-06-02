use egui::{Checkbox, ComboBox, Grid, Ui};

use crate::{
    editor::{
        ModuleUi, modulation_input::ModulationInput, module_label::ModuleLabel,
        stereo_slider::StereoSlider, utils::confirm_module_removal,
    },
    synth_engine::{Input, Lfo, LfoShape, ModuleId, SynthEngine, ui_bridge::UiBridge},
};

impl LfoShape {
    fn label(&self) -> &'static str {
        match self {
            Self::Triangle => "Triangle",
            Self::Square => "Square",
            Self::Sine => "Sine",
        }
    }
}

static SHAPE_OPTIONS: &[LfoShape] = &[LfoShape::Triangle, LfoShape::Square, LfoShape::Sine];

pub struct LfoUi {
    module_id: ModuleId,
    remove_confirmation: bool,
    label_state: Option<String>,
}

impl LfoUi {
    pub fn new(module_id: ModuleId) -> Self {
        Self {
            module_id,
            remove_confirmation: false,
            label_state: None,
        }
    }

    fn lfo<'a>(&mut self, synth: &'a mut SynthEngine) -> &'a mut Lfo {
        synth.get_typed_module_mut(self.module_id).unwrap()
    }
}

impl ModuleUi for LfoUi {
    fn module_id(&self) -> Option<ModuleId> {
        Some(self.module_id)
    }

    fn ui(&mut self, bridge: &mut UiBridge, ui: &mut Ui) {
        let synth = bridge.synth().clone();
        let id = self.module_id;
        let mut ui_data = self.lfo(&mut synth.lock()).get_ui();

        ui.add(ModuleLabel::new(
            &ui_data.label,
            &mut self.label_state,
            &synth,
            self.module_id,
        ));

        ui.add_space(20.0);

        Grid::new("lfo_grid")
            .num_columns(2)
            .spacing([40.0, 24.0])
            .striped(true)
            .show(ui, |ui| {
                ui.label("Shape");
                ComboBox::from_id_salt("shape-select")
                    .selected_text(ui_data.shape.label())
                    .show_ui(ui, |ui| {
                        for shape in SHAPE_OPTIONS {
                            if ui
                                .selectable_label(ui_data.shape == *shape, shape.label())
                                .clicked()
                            {
                                self.lfo(&mut synth.lock()).set_shape(*shape);
                            }
                        }
                    });
                ui.end_row();

                ui.label("Skew");
                if ui
                    .add(ModulationInput::new(
                        &mut ui_data.skew,
                        bridge,
                        Input::Skew,
                        id,
                    ))
                    .changed()
                {
                    self.lfo(&mut synth.lock()).set_skew(ui_data.skew);
                }
                ui.end_row();

                ui.label("Frequency");
                if ui
                    .add(ModulationInput::new(
                        &mut ui_data.frequency,
                        bridge,
                        Input::LowFrequency,
                        id,
                    ))
                    .changed()
                {
                    self.lfo(&mut synth.lock()).set_frequency(ui_data.frequency);
                }
                ui.end_row();

                ui.label("Phase shift");
                if ui
                    .add(ModulationInput::new(
                        &mut ui_data.phase_shift,
                        bridge,
                        Input::PhaseShift,
                        id,
                    ))
                    .changed()
                {
                    self.lfo(&mut synth.lock())
                        .set_phase_shift(ui_data.phase_shift);
                }
                ui.end_row();

                ui.label("Smooth");
                if ui
                    .add(
                        StereoSlider::new(&mut ui_data.smooth_time)
                            .range(0.0..=0.1)
                            .display_scale(1000.0)
                            .default_value(0.0)
                            .skew(1.2)
                            .precision(1)
                            .units(" ms"),
                    )
                    .changed()
                {
                    self.lfo(&mut synth.lock())
                        .set_smooth_time(ui_data.smooth_time);
                }
                ui.end_row();

                ui.label("Bipolar");
                if ui
                    .add(Checkbox::without_text(&mut ui_data.bipolar))
                    .changed()
                {
                    self.lfo(&mut synth.lock()).set_bipolar(ui_data.bipolar);
                }
                ui.end_row();

                ui.label("Steal phase");
                if ui
                    .add(Checkbox::without_text(&mut ui_data.steal_phase))
                    .changed()
                {
                    self.lfo(&mut synth.lock())
                        .set_steal_phase(ui_data.steal_phase);
                }
                ui.end_row();
            });

        ui.add_space(40.0);

        if confirm_module_removal(ui, &mut self.remove_confirmation) {
            synth.lock().remove_module(self.module_id);
        }
    }
}
