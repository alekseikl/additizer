use addi_ui_backend::ui_bridge::modules::segments_envelope::SegmentsEnvelopeUiBridge;
use egui::{Checkbox, ComboBox, Grid, Ui};

use crate::{
    ModuleUi, module_label::ModuleLabel, segments_editor::SegmentsEditor, stereo_input::StereoInput,
};
use addi_engine::segments_envelope::Timebase;
use addi_engine::{Input, ModuleId, ModuleType};
use addi_ui_backend::ui_bridge::{ModuleBridge, UiBridge};

pub struct SegmentsEnvelopeUi {
    module_id: ModuleId,
    editor: SegmentsEditor,
}

impl SegmentsEnvelopeUi {
    pub fn new(module_id: ModuleId) -> Self {
        Self {
            module_id,
            editor: SegmentsEditor::open().with_markers().envelope(),
        }
    }

    fn paint_ui(
        &mut self,
        bridge: &mut UiBridge,
        envelope_bridge: &mut SegmentsEnvelopeUiBridge,
        ui: &mut Ui,
    ) {
        let module_id = self.module_id;

        ui.add(ModuleLabel::new(
            module_id,
            ModuleType::SegmentsEnvelope,
            bridge,
        ));
        ui.add_space(12.0);

        let playhead = if bridge.has_active_voices() {
            let playhead = envelope_bridge.playhead();
            Some([
                (playhead.phase[0], playhead.level[0]),
                (playhead.phase[1], playhead.level[1]),
            ])
        } else {
            None
        };

        if self
            .editor
            .ui_at(ui, envelope_bridge.segments_mut(), playhead)
        {
            envelope_bridge.sync_segments();
        }

        ui.add_space(12.0);

        let mut rate = envelope_bridge.config().rate;
        let mut phase_shift = envelope_bridge.config().phase_shift;
        let mut steal_level = envelope_bridge.config().steal_level;
        let mut keep_voice_alive = envelope_bridge.config().keep_voice_alive;
        let mut timebase = envelope_bridge.config().timebase;

        Grid::new("segments_envelope_grid")
            .num_columns(4)
            .spacing([8.0, 8.0])
            .show(ui, |ui| {
                ui.label("Rate")
                    .on_hover_text("Length of one pass, in the selected unit.");
                if ui
                    .add(StereoInput::new(Input::Rate, module_id, &mut rate, bridge))
                    .changed()
                {
                    envelope_bridge.set_param(Input::Rate, rate);
                }

                ui.label("Timebase");
                ComboBox::from_id_salt(("segments-envelope-timebase", module_id))
                    .selected_text(timebase.label())
                    .show_ui(ui, |ui| {
                        for option in Timebase::ALL {
                            if ui
                                .selectable_value(&mut timebase, option, option.label())
                                .clicked()
                            {
                                envelope_bridge.set_timebase(option);
                            }
                        }
                    });
                ui.end_row();

                ui.label("Phase").on_hover_text(
                    "Starting point on the curve, from 0 to 1. Applied when the voice is triggered.",
                );
                if ui
                    .add(
                        StereoInput::new(Input::Phase, module_id, &mut phase_shift, bridge)
                            .slider(|slider| slider.without_inverse()),
                    )
                    .changed()
                {
                    envelope_bridge.set_param(Input::Phase, phase_shift);
                }
                ui.end_row();

                ui.label("Steal level")
                    .on_hover_text("New voice starts from the previous envelope level");
                if ui.add(Checkbox::without_text(&mut steal_level)).changed() {
                    envelope_bridge.set_steal_level(steal_level);
                }

                ui.label("Keep alive")
                    .on_hover_text("Keeps the voice alive until the envelope finishes");
                if ui
                    .add(Checkbox::without_text(&mut keep_voice_alive))
                    .changed()
                {
                    envelope_bridge.set_keep_voice_alive(keep_voice_alive);
                }
                ui.end_row();
            });
    }
}

impl ModuleUi for SegmentsEnvelopeUi {
    fn module_id(&self) -> Option<ModuleId> {
        Some(self.module_id)
    }

    fn ui(&mut self, bridge: &mut UiBridge, ui: &mut Ui) {
        bridge.with_module_bridge(self.module_id, |bridge, module_bridge| {
            if let ModuleBridge::SegmentsEnvelope(envelope_bridge) = module_bridge {
                self.paint_ui(bridge, envelope_bridge, ui);
            }
        });
    }
}
