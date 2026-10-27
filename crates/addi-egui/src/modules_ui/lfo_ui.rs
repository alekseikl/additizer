use addi_ui_backend::ui_bridge::modules::lfo::LfoUiBridge;
use egui::{Checkbox, ComboBox, Grid, Ui};

use crate::{
    ModuleUi, module_label::ModuleLabel, segments_editor::SegmentsEditor, stereo_input::StereoInput,
};
use addi_engine::lfo::Timebase;
use addi_engine::{Input, ModuleId, ModuleType, StereoSample};
use addi_ui_backend::ui_bridge::{ModuleBridge, UiBridge};

pub struct LfoUi {
    module_id: ModuleId,
    editor: SegmentsEditor,
}

impl LfoUi {
    pub fn new(module_id: ModuleId) -> Self {
        Self {
            module_id,
            editor: SegmentsEditor::default(),
        }
    }

    fn paint_ui(&mut self, bridge: &mut UiBridge, lfo_bridge: &mut LfoUiBridge, ui: &mut Ui) {
        let module_id = self.module_id;

        ui.add(ModuleLabel::new(module_id, ModuleType::Lfo, bridge));
        ui.add_space(12.0);

        let phase = if bridge.has_active_voices() {
            let mut phase_shift = lfo_bridge.config().phase_shift;
            bridge.apply_modulation(module_id, Input::Phase, &mut phase_shift);
            let phase = lfo_bridge.get_phase();
            Some(StereoSample::new(
                (phase[0] + phase_shift[0]).rem_euclid(1.0),
                (phase[1] + phase_shift[1]).rem_euclid(1.0),
            ))
        } else {
            None
        };

        if self.editor.ui(ui, lfo_bridge.segments_mut(), phase) {
            lfo_bridge.sync_segments();
        }

        ui.add_space(12.0);

        let mut frequency = lfo_bridge.config().frequency;
        let mut phase_shift = lfo_bridge.config().phase_shift;
        let mut steal_phase = lfo_bridge.config().steal_phase;
        let mut timebase = lfo_bridge.config().timebase;

        Grid::new("lfo_grid")
            .num_columns(4)
            .spacing([8.0, 8.0])
            .show(ui, |ui| {
                ui.label("Rate");
                if ui
                    .add(StereoInput::new(
                        Input::Rate,
                        module_id,
                        &mut frequency,
                        bridge,
                    ))
                    .changed()
                {
                    lfo_bridge.set_param(Input::Rate, frequency);
                }

                ui.label("Timebase");
                ComboBox::from_id_salt(("lfo-timebase", module_id))
                    .selected_text(timebase.label())
                    .show_ui(ui, |ui| {
                        for option in Timebase::ALL {
                            if ui
                                .selectable_value(&mut timebase, option, option.label())
                                .clicked()
                            {
                                lfo_bridge.set_timebase(option);
                            }
                        }
                    });
                ui.end_row();

                ui.label("Phase");
                if ui
                    .add(StereoInput::new(
                        Input::Phase,
                        module_id,
                        &mut phase_shift,
                        bridge,
                    ))
                    .changed()
                {
                    lfo_bridge.set_param(Input::Phase, phase_shift);
                }

                ui.label("Steal phase");
                if ui.add(Checkbox::without_text(&mut steal_phase)).changed() {
                    lfo_bridge.set_steal_phase(steal_phase);
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
