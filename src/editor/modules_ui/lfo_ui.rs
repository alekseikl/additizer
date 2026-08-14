use egui::{Checkbox, ComboBox, Grid, Ui};

use crate::{
    editor::{
        ModuleUi, module_label::ModuleLabel, slider::Slider, stereo_input::StereoInput,
        units::Units,
    },
    synth_engine::{
        Input, LfoShape, ModuleId, ModuleType,
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
}

impl LfoUi {
    pub fn new(module_id: ModuleId) -> Self {
        Self { module_id }
    }

    fn paint_ui(&mut self, bridge: &mut UiBridge, lfo_bridge: &mut LfoUiBridge, ui: &mut Ui) {
        let module_id = self.module_id;
        let mut config = lfo_bridge.config().clone();

        ui.add(ModuleLabel::new(module_id, ModuleType::Lfo, bridge));

        ui.add_space(16.0);

        Grid::new("lfo_grid")
            .num_columns(4)
            .spacing([8.0, 8.0])
            .show(ui, |ui| {
                ui.label("Shape");
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

                ui.label("Skew");
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

                ui.label("Frequency");
                ui.horizontal(|ui| {
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

                    ui.add_space(8.0);
                });

                ui.label("Phase shift");
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

                ui.label("Smooth");
                if ui
                    .add(
                        Slider::stereo(&mut config.smooth_time, 0.0..=0.1, None)
                            .default(0.0)
                            .skew(1.2)
                            .units(Units::Time),
                    )
                    .changed()
                {
                    lfo_bridge.set_smooth_time(config.smooth_time);
                }

                ui.label("Bipolar");
                if ui
                    .add(Checkbox::without_text(&mut config.bipolar))
                    .changed()
                {
                    lfo_bridge.set_bipolar(config.bipolar);
                }
                ui.end_row();

                ui.label("Steal phase");
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
