use egui_baseview::{
    egui::{
        Button, Checkbox, Color32, ComboBox, Frame, Grid, Id, Label, Modal, RichText, Sense, Sides,
        Slider, TextEdit, Ui, vec2,
    },
    egui_extras::{Column, TableBuilder},
};

use crate::{
    editor::{ModuleUI, multi_input::MultiInput},
    presets::{Preset, PresetInfo, PresetListItem, Presets},
    synth_engine::{Input, ModuleId, OUTPUT_MODULE_ID, SynthEngine, VoiceOverride},
    utils::from_ms,
};

impl VoiceOverride {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Kill => "Kill",
            Self::Steal => "Steal",
        }
    }
}

#[derive(Default)]
pub struct SavePresetState {
    title: String,
    error: String,
}

pub struct LoadPresetState {
    preset_list: Vec<PresetListItem>,
    selected_index: Option<usize>,
    error: String,
}

pub struct ParamsUi {
    save_preset_state: Option<Box<SavePresetState>>,
    load_preset_state: Option<Box<LoadPresetState>>,
}

impl ParamsUi {
    pub fn new() -> Self {
        Self {
            save_preset_state: None,
            load_preset_state: None,
        }
    }

    fn show_save_preset_modal(
        &mut self,
        synth: &mut SynthEngine,
        ui: &mut Ui,
        state: &mut SavePresetState,
    ) -> bool {
        let modal = Modal::new(Id::new("save_preset_modal")).show(ui.ctx(), |ui| {
            ui.set_width(220.0);
            ui.heading("Save Preset");
            ui.add_space(16.0);
            ui.horizontal(|ui| {
                ui.label("Title");
                ui.add(TextEdit::singleline(&mut state.title))
                    .request_focus();
            });

            if !state.error.is_empty() {
                ui.label(RichText::new(&state.error).color(Color32::RED));
            }

            ui.add_space(32.0);

            let trimmed = state.title.trim();
            let valid = !trimmed.is_empty();

            Sides::new().show(
                ui,
                |_ui| {},
                |ui| {
                    if ui.add_enabled(valid, Button::new("Save")).clicked() {
                        let config = synth.get_config();
                        let preset = Preset {
                            info: PresetInfo {
                                title: trimmed.to_string(),
                            },
                            config,
                        };

                        if let Some(presets) = Presets::new() {
                            if presets.write_preset(&preset).is_some() {
                                state.error = String::new();
                                ui.close();
                            } else {
                                state.error = "Failed to save preset.".into();
                            }
                        }
                    }

                    if ui.button("Cancel").clicked() {
                        ui.close();
                    }
                },
            );
        });

        !modal.should_close()
    }

    fn show_load_preset_modal(
        &mut self,
        synth: &mut SynthEngine,
        ui: &mut Ui,
        state: &mut LoadPresetState,
    ) -> bool {
        let modal = Modal::new(Id::new("load_preset_modal")).show(ui.ctx(), |ui| {
            ui.set_width(440.0);

            ui.heading("Load Preset");
            ui.add_space(16.0);

            Frame::new().inner_margin(vec2(8.0, 8.0)).show(ui, |ui| {
                let table = TableBuilder::new(ui)
                    .striped(true)
                    .resizable(false)
                    .sense(Sense::click())
                    .min_scrolled_height(0.0)
                    .max_scroll_height(300.0)
                    .column(Column::remainder());

                table
                    .header(20.0, |mut header| {
                        header.col(|ui| {
                            ui.strong("Title");
                        });
                    })
                    .body(|mut body| {
                        for (idx, preset) in state.preset_list.iter().enumerate() {
                            body.row(18.0, |mut row| {
                                row.set_selected(state.selected_index == Some(idx));

                                row.col(|ui| {
                                    ui.add(Label::new(&preset.info.title).selectable(false));
                                });

                                if row.response().clicked() {
                                    state.selected_index = Some(idx);
                                }
                            });
                        }
                    });

                if !state.error.is_empty() {
                    ui.label(RichText::new(&state.error).color(Color32::RED));
                }
            });

            Sides::new().show(
                ui,
                |_ui| {},
                |ui| {
                    if ui
                        .add_enabled(state.selected_index.is_some(), Button::new("Load"))
                        .clicked()
                    {
                        if let Some(idx) = state.selected_index
                            && let Some(preset) = Presets::read_preset(&state.preset_list[idx].path)
                            && synth.set_config(&preset.config)
                        {
                            ui.close();
                        } else {
                            state.error = "Failed to load preset.".into();
                        }
                    }

                    if ui.button("Cancel").clicked() {
                        ui.close();
                    }
                },
            );
        });

        !modal.should_close()
    }
}

impl ModuleUI for ParamsUi {
    fn module_id(&self) -> Option<ModuleId> {
        None
    }

    fn ui(&mut self, synth: &mut SynthEngine, ui: &mut Ui) {
        ui.heading("Parameters");
        ui.add_space(20.0);

        Grid::new("params_grid")
            .num_columns(2)
            .spacing([40.0, 24.0])
            .striped(true)
            .show(ui, |ui| {
                let block_sizes = [8, 16, 32, 64, 128];
                let mut ui_data = synth.get_ui();
                let mut kill_time_ms = ui_data.voice_kill_time * 1000.0;

                ui.label("Voices");
                if ui
                    .add(Slider::new(
                        &mut ui_data.voices,
                        1..=SynthEngine::AVAILABLE_VOICES,
                    ))
                    .changed()
                {
                    synth.set_num_voices(ui_data.voices);
                }
                ui.end_row();

                let overrides = [VoiceOverride::Kill, VoiceOverride::Steal];

                ui.label("Voice override");
                ComboBox::from_id_salt("voice-override-select")
                    .selected_text(ui_data.voice_override.label())
                    .show_ui(ui, |ui| {
                        for vo in &overrides {
                            if ui
                                .selectable_value(&mut ui_data.voice_override, *vo, vo.label())
                                .clicked()
                            {
                                synth.set_voice_override(*vo);
                            }
                        }
                    });
                ui.end_row();

                ui.label("Voice kill time");
                if ui
                    .add(Slider::new(&mut kill_time_ms, 4.0..=100.0))
                    .changed()
                {
                    synth.set_voice_kill_time(from_ms(kill_time_ms));
                }
                ui.end_row();

                ui.label("Voices state");
                ui.label(format!(
                    "Playing: {:02}, Releasing: {:02}, Killing: {:02}",
                    ui_data.playing_voices, ui_data.releasing_voices, ui_data.killing_voices
                ));
                ui.end_row();

                ui.label("Block Size");
                ComboBox::from_id_salt("buff-size-select")
                    .selected_text(format!("{} samples", ui_data.block_size))
                    .show_ui(ui, |ui| {
                        for sz in &block_sizes {
                            if ui
                                .selectable_value(
                                    &mut ui_data.block_size,
                                    *sz,
                                    format!("{} samples", sz),
                                )
                                .clicked()
                            {
                                synth.set_block_size(*sz);
                            }
                        }
                    });
                ui.end_row();

                ui.label("Oversampling x2");
                if ui
                    .add(Checkbox::without_text(&mut ui_data.oversampling))
                    .changed()
                {
                    synth.set_oversampling(ui_data.oversampling);
                }
                ui.end_row();

                ui.label("Output");
                ui.add(MultiInput::new(synth, Input::Audio, OUTPUT_MODULE_ID));
                ui.end_row();

                ui.label("Presets");
                ui.horizontal(|ui| {
                    if ui.button("Save").clicked() {
                        self.save_preset_state = Some(Box::new(SavePresetState::default()));
                    }

                    if ui.button("Load").clicked()
                        && let Some(presets) = Presets::new()
                    {
                        self.load_preset_state = Some(Box::new(LoadPresetState {
                            preset_list: presets.read_presets_list(),
                            selected_index: None,
                            error: String::new(),
                        }));
                    }
                });
                ui.end_row();
            });

        if let Some(mut state) = self.save_preset_state.take()
            && self.show_save_preset_modal(synth, ui, &mut state)
        {
            self.save_preset_state.replace(state);
        }

        if let Some(mut state) = self.load_preset_state.take()
            && self.show_load_preset_modal(synth, ui, &mut state)
        {
            self.load_preset_state.replace(state);
        }
    }
}
