use egui::{Checkbox, Grid, Ui};

use crate::{
    editor::{ModuleUi, module_label::ModuleLabel, stereo_input::StereoInput},
    synth_engine::{
        Input, ModuleId, ModuleType,
        pitch::PitchUiBridge,
        ui_bridge::{ModuleBridge, UiBridge},
    },
};

pub struct PitchUi {
    module_id: ModuleId,
}

impl PitchUi {
    pub fn new(module_id: ModuleId) -> Self {
        Self { module_id }
    }

    fn paint_ui(&mut self, bridge: &mut UiBridge, pitch_bridge: &mut PitchUiBridge, ui: &mut Ui) {
        let module_id = self.module_id;
        let mut config = pitch_bridge.config().clone();

        ui.add(ModuleLabel::new(module_id, ModuleType::Pitch, bridge));

        ui.add_space(16.0);

        Grid::new("pitch_grid")
            .num_columns(4)
            .spacing([8.0, 8.0])
            .show(ui, |ui| {
                ui.label("Pitch shift");
                if ui
                    .add(StereoInput::new(
                        Input::PitchShift,
                        module_id,
                        &mut config.pitch_shift,
                        bridge,
                    ))
                    .changed()
                {
                    pitch_bridge.set_param(Input::PitchShift, config.pitch_shift);
                }

                ui.label("Keytrack");
                if ui
                    .add(Checkbox::without_text(&mut config.keytrack))
                    .changed()
                {
                    pitch_bridge.set_keytrack(config.keytrack);
                }
                ui.end_row();

                ui.label("Glide");
                if ui
                    .add(StereoInput::new(
                        Input::Glide,
                        module_id,
                        &mut config.glide,
                        bridge,
                    ))
                    .changed()
                {
                    pitch_bridge.set_param(Input::Glide, config.glide);
                }

                ui.label("Glide always");
                if ui
                    .add(Checkbox::without_text(&mut config.glide_always))
                    .changed()
                {
                    pitch_bridge.set_glide_always(config.glide_always);
                }
                ui.end_row();

                ui.label("Glide slope");
                if ui
                    .add(StereoInput::new(
                        Input::GlideSlope,
                        module_id,
                        &mut config.glide_slope,
                        bridge,
                    ))
                    .changed()
                {
                    pitch_bridge.set_param(Input::GlideSlope, config.glide_slope);
                }

                ui.label("Glide per octave");
                if ui
                    .add(Checkbox::without_text(&mut config.glide_per_octave))
                    .changed()
                {
                    pitch_bridge.set_glide_per_octave(config.glide_per_octave);
                }
                ui.end_row();
            });
    }
}

impl ModuleUi for PitchUi {
    fn module_id(&self) -> Option<ModuleId> {
        Some(self.module_id)
    }

    fn ui(&mut self, bridge: &mut UiBridge, ui: &mut Ui) {
        bridge.with_module_bridge(self.module_id, |bridge, module_bridge| {
            if let ModuleBridge::Pitch(pitch_bridge) = module_bridge {
                self.paint_ui(bridge, pitch_bridge, ui);
            }
        });
    }
}
