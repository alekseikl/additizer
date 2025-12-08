use egui_baseview::egui::{Checkbox, ComboBox, Grid, Ui};

use crate::{
    editor::{
        ModuleUI, modulation_input::ModulationInput, module_label::ModuleLabel,
        utils::confirm_module_removal,
    },
    synth_engine::{Input, Lfo, LfoShape, ModuleId, SynthEngine},
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
        Lfo::downcast_mut_unwrap(synth.get_module_mut(self.module_id))
    }
}

impl ModuleUI for LfoUi {
    fn module_id(&self) -> ModuleId {
        self.module_id
    }

    fn ui(&mut self, synth: &mut SynthEngine, ui: &mut Ui) {
        let id = self.module_id;
        let mut ui_data = self.lfo(synth).get_ui();

        ui.add(ModuleLabel::new(
            &ui_data.label,
            &mut self.label_state,
            synth.get_module_mut(self.module_id).unwrap(),
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
                                self.lfo(synth).set_shape(*shape);
                            }
                        }
                    });
                ui.end_row();

                ui.label("Skew");
                if ui
                    .add(ModulationInput::new(
                        &mut ui_data.skew,
                        synth,
                        Input::Skew,
                        id,
                    ))
                    .changed()
                {
                    self.lfo(synth).set_skew(ui_data.skew);
                }
                ui.end_row();

                ui.label("Frequency");
                if ui
                    .add(ModulationInput::new(
                        &mut ui_data.frequency,
                        synth,
                        Input::LowFrequency,
                        id,
                    ))
                    .changed()
                {
                    self.lfo(synth).set_frequency(ui_data.frequency);
                }
                ui.end_row();

                ui.label("Phase shift");
                if ui
                    .add(ModulationInput::new(
                        &mut ui_data.phase_shift,
                        synth,
                        Input::PhaseShift,
                        id,
                    ))
                    .changed()
                {
                    self.lfo(synth).set_phase_shift(ui_data.phase_shift);
                }
                ui.end_row();

                ui.label("Bipolar");
                if ui
                    .add(Checkbox::without_text(&mut ui_data.bipolar))
                    .changed()
                {
                    self.lfo(synth).set_bipolar(ui_data.bipolar);
                }
                ui.end_row();

                ui.label("Reset phase");
                if ui
                    .add(Checkbox::without_text(&mut ui_data.reset_phase))
                    .changed()
                {
                    self.lfo(synth).set_reset_phase(ui_data.reset_phase);
                }
                ui.end_row();
            });

        ui.add_space(40.0);

        if confirm_module_removal(ui, &mut self.remove_confirmation) {
            synth.remove_module(self.module_id);
        }
    }
}
