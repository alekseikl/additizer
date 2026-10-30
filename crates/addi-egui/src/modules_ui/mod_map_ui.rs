use addi_ui_backend::ui_bridge::modules::mod_map::ModMapUiBridge;
use egui::{Checkbox, Grid, Ui};

use crate::{
    ModuleUi, module_label::ModuleLabel, segments_editor::SegmentsEditor, slider::Slider,
    stereo_input::StereoInput, units::Units,
};
use addi_engine::mod_map::MAX_SMOOTH;
use addi_engine::{Input, ModuleId, ModuleType};
use addi_ui_backend::ui_bridge::{ModuleBridge, UiBridge};

pub struct ModMapUi {
    module_id: ModuleId,
    editor: SegmentsEditor,
}

impl ModMapUi {
    pub fn new(module_id: ModuleId) -> Self {
        Self {
            module_id,
            editor: SegmentsEditor::lookup(),
        }
    }

    fn paint_ui(&mut self, bridge: &mut UiBridge, map_bridge: &mut ModMapUiBridge, ui: &mut Ui) {
        let module_id = self.module_id;

        ui.add(ModuleLabel::new(module_id, ModuleType::ModMap, bridge));
        ui.add_space(12.0);

        if self.editor.ui(ui, map_bridge.segments_mut(), None) {
            map_bridge.sync_segments();
        }

        ui.add_space(12.0);

        let mut amount = map_bridge.config().amount;
        let mut bipolar = map_bridge.config().bipolar;
        let mut smooth = map_bridge.config().smooth;

        Grid::new(("mod-map", module_id))
            .num_columns(4)
            .spacing([8.0, 8.0])
            .show(ui, |ui| {
                ui.label("Amount");
                if ui
                    .add(StereoInput::new(
                        Input::Amount,
                        module_id,
                        &mut amount,
                        bridge,
                    ))
                    .changed()
                {
                    map_bridge.set_param(Input::Amount, amount);
                }

                ui.label("Bipolar")
                    .on_hover_text("Remaps [0, 1] into [-1, 1]");
                if ui.add(Checkbox::without_text(&mut bipolar)).changed() {
                    map_bridge.set_bipolar(bipolar);
                }
                ui.end_row();

                ui.label("Smooth");
                if ui
                    .add(
                        Slider::mono(&mut smooth, 0.0..=MAX_SMOOTH, None)
                            .default(0.0)
                            .skew(1.2)
                            .units(Units::Time),
                    )
                    .changed()
                {
                    map_bridge.set_smooth(smooth);
                }
                ui.end_row();
            });
    }
}

impl ModuleUi for ModMapUi {
    fn module_id(&self) -> Option<ModuleId> {
        Some(self.module_id)
    }

    fn ui(&mut self, bridge: &mut UiBridge, ui: &mut Ui) {
        bridge.with_module_bridge(self.module_id, |bridge, module_bridge| {
            if let ModuleBridge::ModMap(map_bridge) = module_bridge {
                self.paint_ui(bridge, map_bridge, ui);
            }
        });
    }
}
