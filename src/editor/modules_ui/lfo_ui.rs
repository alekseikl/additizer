use egui::{Align, Checkbox, ComboBox, Grid, Layout, Ui};

use crate::{
    editor::{
        ModuleUi,
        module_label::ModuleLabel,
        slider::{self, Slider},
        stereo_input::StereoInput,
    },
    synth_engine::{
        Input, LfoShape, ModuleId,
        lfo::LfoUiBridge,
        ui_bridge::{ModuleBridge, UiBridge},
    },
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
    label_state: Option<String>,
}

impl LfoUi {
    pub fn new(module_id: ModuleId) -> Self {
        Self {
            module_id,
            label_state: None,
        }
    }

    fn paint_ui(&mut self, bridge: &mut UiBridge, lfo_bridge: &mut LfoUiBridge, ui: &mut Ui) {
        let module_id = self.module_id;
        let mut config = lfo_bridge.config().clone();

        ui.add(ModuleLabel::new(&mut self.label_state, bridge, module_id));

        ui.add_space(20.0);

        let label = |ui: &mut Ui, text: &str| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(text);
            });
        };

        Grid::new("lfo_grid")
            .num_columns(4)
            .spacing([8.0, 8.0])
            .show(ui, |ui| {
                label(ui, "Shape");
                ComboBox::from_id_salt("shape-select")
                    .selected_text(config.shape.label())
                    .show_ui(ui, |ui| {
                        for shape in SHAPE_OPTIONS {
                            if ui
                                .selectable_label(config.shape == *shape, shape.label())
                                .clicked()
                            {
                                lfo_bridge.set_shape(*shape);
                            }
                        }
                    });

                label(ui, "Skew");
                if ui
                    .add(StereoInput::new(
                        Input::Skew,
                        module_id,
                        &mut config.skew,
                        bridge,
                    ))
                    .changed()
                {
                    lfo_bridge.set_param(Input::Skew, config.skew);
                }
                ui.end_row();

                label(ui, "Frequency");
                if ui
                    .add(StereoInput::new(
                        Input::LowFrequency,
                        module_id,
                        &mut config.frequency,
                        bridge,
                    ))
                    .changed()
                {
                    lfo_bridge.set_param(Input::LowFrequency, config.frequency);
                }

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
                    lfo_bridge.set_param(Input::PhaseShift, config.phase_shift);
                }
                ui.end_row();

                label(ui, "Smooth");
                if ui
                    .add(
                        Slider::stereo(&mut config.smooth_time, 0.0..=0.1, None)
                            .default(0.0)
                            .skew(1.2)
                            .units(slider::Units::Time),
                    )
                    .changed()
                {
                    lfo_bridge.set_smooth_time(config.smooth_time);
                }

                label(ui, "Bipolar");
                if ui
                    .add(Checkbox::without_text(&mut config.bipolar))
                    .changed()
                {
                    lfo_bridge.set_bipolar(config.bipolar);
                }
                ui.end_row();

                label(ui, "Steal phase");
                if ui
                    .add(Checkbox::without_text(&mut config.steal_phase))
                    .changed()
                {
                    lfo_bridge.set_steal_phase(config.steal_phase);
                }
                ui.end_row();
            });
    }
}

impl ModuleUi for LfoUi {
    fn module_id(&self) -> Option<ModuleId> {
        Some(self.module_id)
    }

    fn ui(&mut self, bridge: &mut UiBridge, ui: &mut Ui) {
        bridge.with_module_bridge(self.module_id, |bridge, module_bridge| {
            if let ModuleBridge::Lfo(lfo_bridge) = module_bridge {
                self.paint_ui(bridge, lfo_bridge, ui);
            }
        });
    }
}
