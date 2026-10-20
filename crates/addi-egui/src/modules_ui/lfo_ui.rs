use addi_ui_backend::ui_bridge::modules::lfo::LfoUiBridge;
use egui::{Checkbox, Grid, Ui};

use crate::{
    ModuleUi, line_editor::LineEditor, module_label::ModuleLabel, stereo_input::StereoInput,
};
use addi_engine::{Input, ModuleId, ModuleType};
use addi_ui_backend::ui_bridge::{ModuleBridge, UiBridge};

pub struct LfoUi {
    module_id: ModuleId,
    editor: LineEditor,
}

impl LfoUi {
    pub fn new(module_id: ModuleId) -> Self {
        Self {
            module_id,
            editor: LineEditor::default(),
        }
    }

    fn paint_ui(&mut self, bridge: &mut UiBridge, lfo_bridge: &mut LfoUiBridge, ui: &mut Ui) {
        let module_id = self.module_id;
        let mut config = lfo_bridge.config().clone();

        ui.add(ModuleLabel::new(module_id, ModuleType::Lfo, bridge));
        ui.add_space(12.0);

        let phase = if bridge.has_active_voices() {
            let mut phase_shift = config.phase_shift;
            bridge.apply_modulation(module_id, Input::PhaseShift, &mut phase_shift);
            Some((lfo_bridge.get_phase() + phase_shift[0]).rem_euclid(1.0))
        } else {
            None
        };

        if self.editor.ui(ui, &mut config.line, phase) {
            lfo_bridge.set_line(config.line);
        }

        ui.add_space(12.0);

        Grid::new("lfo_grid")
            .num_columns(4)
            .spacing([8.0, 8.0])
            .show(ui, |ui| {
                ui.label("Frequency");
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
