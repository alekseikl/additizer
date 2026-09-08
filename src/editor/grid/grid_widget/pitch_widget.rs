use egui::{Align, Label, Layout, RichText};

use crate::{
    editor::grid::WidgetCtx,
    synth_engine::{
        ModuleId, Sample,
        ui_bridge::{GridVec, ModuleBridge},
    },
};

use super::GridWidgetContent;

/// Pitch in octave units (relative to A4) → note name, e.g. "C4", "A#3".
fn pitch_to_note_name(pitch: Sample) -> String {
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];

    let note = (pitch * 12.0 + 69.0).round() as i32;
    let name = NAMES[note.rem_euclid(12) as usize];
    let octave = note.div_euclid(12) - 1;

    format!("{name}{octave}")
}

#[derive(Default)]
pub struct PitchWidget {}

impl GridWidgetContent for PitchWidget {
    fn grid_size(&self) -> GridVec {
        GridVec::new(2, 2)
    }

    fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut WidgetCtx, module_id: ModuleId) {
        let has_active_voices = ctx.bridge.has_active_voices();

        ctx.bridge
            .with_module_bridge(module_id, |_bridge, module_bridge| {
                if let ModuleBridge::Pitch(pitch_bridge) = module_bridge {
                    let text = if has_active_voices {
                        pitch_to_note_name(pitch_bridge.get_pitch())
                    } else {
                        "—".to_string()
                    };

                    ui.add_space(8.0);

                    ui.with_layout(Layout::top_down_justified(Align::Center), |ui| {
                        ui.add(Label::new(RichText::new(text).size(18.0)).selectable(false));
                    });
                }
            });
    }
}
