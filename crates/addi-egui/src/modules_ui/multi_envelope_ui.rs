use addi_ui_backend::ui_bridge::modules::multi_envelope::MultiEnvelopeUiBridge;
use egui::{Checkbox, Grid, Ui};

use crate::{ModuleUi, module_label::ModuleLabel, time_segments_editor::TimeSegmentsEditor};
use addi_engine::{ModuleId, ModuleType};
use addi_ui_backend::ui_bridge::{ModuleBridge, UiBridge};

pub struct MultiEnvelopeUi {
    module_id: ModuleId,
    editor: TimeSegmentsEditor,
}

impl MultiEnvelopeUi {
    pub fn new(module_id: ModuleId) -> Self {
        Self {
            module_id,
            editor: TimeSegmentsEditor::default(),
        }
    }

    fn paint_ui(
        &mut self,
        bridge: &mut UiBridge,
        envelope_bridge: &mut MultiEnvelopeUiBridge,
        ui: &mut Ui,
    ) {
        let module_id = self.module_id;

        ui.add(ModuleLabel::new(
            module_id,
            ModuleType::MultiEnvelope,
            bridge,
        ));
        ui.add_space(12.0);

        let playhead = if bridge.has_active_voices() {
            let playhead = envelope_bridge.playhead();
            Some([
                (playhead.time[0], playhead.level[0]),
                (playhead.time[1], playhead.level[1]),
            ])
        } else {
            None
        };

        if self.editor.ui(ui, envelope_bridge.segments_mut(), playhead) {
            envelope_bridge.sync_segments();
        }

        ui.add_space(12.0);

        let mut steal_level = envelope_bridge.config().steal_level;
        let mut keep_voice_alive = envelope_bridge.config().keep_voice_alive;
        let has_release = envelope_bridge
            .config()
            .segments
            .release_point_idx()
            .is_some();

        Grid::new("multi_envelope_grid")
            .num_columns(2)
            .spacing([8.0, 8.0])
            .show(ui, |ui| {
                if ui
                    .add(Checkbox::new(&mut steal_level, "Steal level"))
                    .on_hover_text("New voice starts from the previous envelope level")
                    .changed()
                {
                    envelope_bridge.set_steal_level(steal_level);
                }

                if ui
                    .add_enabled(
                        has_release,
                        Checkbox::new(&mut keep_voice_alive, "Keep alive"),
                    )
                    .on_hover_text("Keeps the voice alive until the release finishes")
                    .on_disabled_hover_text(
                        "Set a release point to hold the voice through the release",
                    )
                    .changed()
                {
                    envelope_bridge.set_keep_voice_alive(keep_voice_alive);
                }
                ui.end_row();
            });
    }
}

impl ModuleUi for MultiEnvelopeUi {
    fn module_id(&self) -> Option<ModuleId> {
        Some(self.module_id)
    }

    fn ui(&mut self, bridge: &mut UiBridge, ui: &mut Ui) {
        bridge.with_module_bridge(self.module_id, |bridge, module_bridge| {
            if let ModuleBridge::MultiEnvelope(envelope_bridge) = module_bridge {
                self.paint_ui(bridge, envelope_bridge, ui);
            }
        });
    }
}
