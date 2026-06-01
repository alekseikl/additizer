use egui::{
    Button, Checkbox, Color32, ComboBox, Frame, Grid, Id, Label, Modal, RichText, Sense, Sides,
    Slider, TextEdit, Ui, vec2,
};
use egui_extras::{Column, TableBuilder};

use crate::{
    editor::{ModuleUi, SynthEngineHandle, multi_input::MultiInput},
    presets::{Preset, PresetInfo, PresetListItem, Presets},
    synth_engine::{Input, ModuleId, OUTPUT_MODULE_ID, SynthEngine, ui_bridge::UiBridge},
    utils::from_ms,
};

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
        synth: &SynthEngineHandle,
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
                        let config = synth.lock().get_config();
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
        synth: &SynthEngineHandle,
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
                            && synth.lock().set_config(&preset.config)
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

impl ModuleUi for ParamsUi {
    fn module_id(&self) -> Option<ModuleId> {
        None
    }

    fn ui(&mut self, bridge: &mut UiBridge, ui: &mut Ui) {
        ui.heading("Parameters");
        ui.add_space(20.0);

        Grid::new("params_grid")
            .num_columns(2)
            .spacing([40.0, 24.0])
            .striped(true)
            .show(ui, |ui| {
                let controls = bridge.controls();
                let block_sizes = [8, 16, 32, 64, 128];
                let voices_status = *bridge.voices_status();

                let mut kill_time_ms = controls.voice_kill_time * 1000.0;
                let mut voices = controls.voices;
                let mut legato = controls.legato;
                let mut block_size = controls.block_size;
                let mut oversampling = controls.oversampling;
                let mut stereo_spectrum = controls.stereo_spectrum;

                ui.label("Voices");
                if ui
                    .add(Slider::new(&mut voices, 1..=SynthEngine::AVAILABLE_VOICES))
                    .changed()
                {
                    bridge.set_voices(voices);
                }
                ui.end_row();

                ui.label("Legato");
                if ui.add(Checkbox::without_text(&mut legato)).changed() {
                    bridge.set_legato(legato);
                }
                ui.end_row();

                ui.label("Voice kill time");
                if ui
                    .add(Slider::new(&mut kill_time_ms, 4.0..=100.0))
                    .changed()
                {
                    bridge.set_voice_kill_time(from_ms(kill_time_ms));
                }
                ui.end_row();

                ui.label("Voices state");
                ui.label(format!(
                    "Playing: {:02}, Releasing: {:02}, Killing: {:02}, Waiting Notes: {:02}",
                    voices_status.playing,
                    voices_status.releasing,
                    voices_status.killing,
                    voices_status.waiting_notes
                ));
                ui.end_row();

                ui.label("Block Size");
                ComboBox::from_id_salt("buff-size-select")
                    .selected_text(format!("{} samples", block_size))
                    .show_ui(ui, |ui| {
                        for sz in &block_sizes {
                            if ui
                                .selectable_value(&mut block_size, *sz, format!("{} samples", sz))
                                .clicked()
                            {
                                bridge.set_block_size(*sz);
                            }
                        }
                    });
                ui.end_row();

                ui.label("Oversampling x2");
                if ui.add(Checkbox::without_text(&mut oversampling)).changed() {
                    bridge.set_oversampling(oversampling);
                }
                ui.end_row();

                ui.label("Stereo Spectrum");
                if ui
                    .add(Checkbox::without_text(&mut stereo_spectrum))
                    .changed()
                {
                    bridge.set_stereo_spectrum(stereo_spectrum);
                }
                ui.end_row();

                ui.label("Output");
                MultiInput::new(Input::Audio, OUTPUT_MODULE_ID).show(ui, bridge);
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
            && self.show_save_preset_modal(bridge.synth(), ui, &mut state)
        {
            self.save_preset_state.replace(state);
        }

        if let Some(mut state) = self.load_preset_state.take()
            && self.show_load_preset_modal(bridge.synth(), ui, &mut state)
        {
            self.load_preset_state.replace(state);
        }
    }
}
