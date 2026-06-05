use egui::{Checkbox, ComboBox, Grid, Ui};

use crate::{
    editor::{
        ModuleUi, modulation_input::ModulationInput, module_label::ModuleLabel,
        stereo_slider::StereoSlider, utils::confirm_module_removal,
    },
    synth_engine::{Input, LfoShape, ModuleId, lfo, ui_bridge::UiBridge},
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
    remove_confirmation: bool,
    label_state: Option<String>,
    lfo_bridge: lfo::UiBridge,
}

impl LfoUi {
    pub fn new(module_id: ModuleId, synth_bridge: &mut UiBridge) -> Option<Self> {
        let lfo_bridge = lfo::UiBridge::create(module_id, synth_bridge.synth().clone())?;

        Some(Self {
            remove_confirmation: false,
            label_state: None,
            lfo_bridge,
        })
    }
}

impl ModuleUi for LfoUi {
    fn module_id(&self) -> Option<ModuleId> {
        Some(self.lfo_bridge.module_id())
    }

    fn ui(&mut self, bridge: &mut UiBridge, ui: &mut Ui) {
        let module_id = self.lfo_bridge.module_id();
        let mut config = self.lfo_bridge.config().clone();

        ui.add(ModuleLabel::new(
            &mut self.label_state,
            bridge,
            module_id,
        ));

        ui.add_space(20.0);

        Grid::new("lfo_grid")
            .num_columns(2)
            .spacing([40.0, 24.0])
            .striped(true)
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
                                self.lfo_bridge.set_shape(*shape);
                            }
                        }
                    });
                ui.end_row();

                ui.label("Skew");
                if ui
                    .add(ModulationInput::new(
                        &mut config.skew,
                        bridge,
                        Input::Skew,
                        module_id,
                    ))
                    .changed()
                {
                    self.lfo_bridge.set_param(Input::Skew, config.skew);
                }
                ui.end_row();

                ui.label("Frequency");
                if ui
                    .add(ModulationInput::new(
                        &mut config.frequency,
                        bridge,
                        Input::LowFrequency,
                        module_id,
                    ))
                    .changed()
                {
                    self.lfo_bridge.set_param(Input::LowFrequency, config.frequency);
                }
                ui.end_row();

                ui.label("Phase shift");
                if ui
                    .add(ModulationInput::new(
                        &mut config.phase_shift,
                        bridge,
                        Input::PhaseShift,
                        module_id,
                    ))
                    .changed()
                {
                    self.lfo_bridge
                        .set_param(Input::PhaseShift, config.phase_shift);
                }
                ui.end_row();

                ui.label("Smooth");
                if ui
                    .add(
                        StereoSlider::new(&mut config.smooth_time)
                            .range(0.0..=0.1)
                            .display_scale(1000.0)
                            .default_value(0.0)
                            .skew(1.2)
                            .precision(1)
                            .units(" ms"),
                    )
                    .changed()
                {
                    self.lfo_bridge.set_smooth_time(config.smooth_time);
                }
                ui.end_row();

                ui.label("Bipolar");
                if ui
                    .add(Checkbox::without_text(&mut config.bipolar))
                    .changed()
                {
                    self.lfo_bridge.set_bipolar(config.bipolar);
                }
                ui.end_row();

                ui.label("Steal phase");
                if ui
                    .add(Checkbox::without_text(&mut config.steal_phase))
                    .changed()
                {
                    self.lfo_bridge.set_steal_phase(config.steal_phase);
                }
                ui.end_row();
            });

        ui.add_space(40.0);

        if confirm_module_removal(ui, &mut self.remove_confirmation) {
            bridge.remove_module(module_id);
        }
    }
}
