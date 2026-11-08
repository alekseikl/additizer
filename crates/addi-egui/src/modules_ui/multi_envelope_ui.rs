use addi_ui_backend::ui_bridge::modules::multi_envelope::MultiEnvelopeUiBridge;
use egui::{Align, Checkbox, FontFamily, Grid, Layout, RichText, Ui};

use crate::{
    ModuleUi, module_label::ModuleLabel, segments_editor::TimeSegmentsEditor,
    stereo_input::StereoInput,
};
use addi_engine::{Input, ModuleId, ModuleType};
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

        let mut steal_level = envelope_bridge.config().steal_level;
        let mut keep_voice_alive = envelope_bridge.config().keep_voice_alive;
        let has_release = envelope_bridge
            .config()
            .segments
            .release_point_idx()
            .is_some();
        let mut steal_level_changed = false;
        let mut keep_voice_alive_changed = false;

        if self
            .editor
            .ui(ui, envelope_bridge.segments_mut(), playhead, |ui| {
                steal_level_changed = ui
                    .add(Checkbox::new(&mut steal_level, "Steal level"))
                    .on_hover_text("New voice starts from the previous envelope level")
                    .changed();

                ui.add_space(8.0);

                keep_voice_alive_changed = ui
                    .add_enabled(
                        has_release,
                        Checkbox::new(&mut keep_voice_alive, "Keep alive"),
                    )
                    .on_hover_text("Keeps the voice alive until the release finishes")
                    .on_disabled_hover_text(
                        "Set a release point to hold the voice through the release",
                    )
                    .changed();
            })
        {
            envelope_bridge.sync_segments();
        }

        if steal_level_changed {
            envelope_bridge.set_steal_level(steal_level);
        }

        if keep_voice_alive_changed {
            envelope_bridge.set_keep_voice_alive(keep_voice_alive);
        }

        ui.add_space(12.0);

        let num_points = envelope_bridge.config().segments.num_points();

        Grid::new(("multi_envelope_offsets", module_id))
            .num_columns(3)
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
                ui.label(RichText::new("Point").family(FontFamily::Name("Bold".into())));
                ui.label(RichText::new("Duration offset").family(FontFamily::Name("Bold".into())));
                ui.label(RichText::new("Level offset").family(FontFamily::Name("Bold".into())));
                ui.end_row();

                for index in 0..num_points {
                    let last = index + 1 == num_points;
                    let mut duration = envelope_bridge.config().duration_offset(index);
                    let mut level = envelope_bridge.config().level_offset(index);
                    let index = index as u8;

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!("#{}", index + 1))
                                .family(FontFamily::Name("Bold".into())),
                        );
                    });

                    if last {
                        ui.label("");
                    } else if ui
                        .add(StereoInput::new(
                            Input::DurationMix(index),
                            module_id,
                            &mut duration,
                            bridge,
                        ))
                        .changed()
                    {
                        envelope_bridge.set_param(Input::DurationMix(index), duration);
                    }

                    if ui
                        .add(
                            StereoInput::new(
                                Input::LinLevelMix(index),
                                module_id,
                                &mut level,
                                bridge,
                            )
                            .default(0.0),
                        )
                        .changed()
                    {
                        envelope_bridge.set_param(Input::LinLevelMix(index), level);
                    }
                    ui.end_row();
                }
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
