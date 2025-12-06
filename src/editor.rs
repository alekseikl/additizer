use std::sync::Arc;

use egui_baseview::egui::{
    CentralPanel, Color32, ComboBox, Frame, Grid, Margin, Response, ScrollArea, Sense, Separator,
    SidePanel, Slider, TopBottomPanel, Ui, Vec2, vec2,
};
use nih_plug::editor::Editor;
use parking_lot::Mutex;

use crate::{
    editor::{
        gain_slider::GainSlider,
        modules_ui::{
            AmplifierUI, EnvelopeUI, ExternalParamUI, HarmonicEditorUI, LfoUi, ModulationFilterUI,
            OscillatorUI, SpectralBlendUi, SpectralFilterUI,
        },
    },
    synth_engine::{
        Input, ModuleId, ModuleInput, ModuleType, OUTPUT_MODULE_ID, SynthEngine, SynthModule,
        VoiceOverride,
    },
};

use egui_integration::{ResizableWindow, create_egui_editor};

pub use egui_integration::EguiState;

mod direct_input;
mod egui_integration;
mod gain_slider;
mod modulation_input;
mod module_label;
mod modules_ui;
mod multi_input;
mod stereo_slider;
mod utils;

pub trait ModuleUI {
    fn module_id(&self) -> ModuleId;
    fn ui(&mut self, synth: &mut SynthEngine, ui: &mut Ui);
}

type ModuleUIBox = Box<dyn ModuleUI + Send + Sync>;

struct EditorState {
    selected_module_ui: Option<ModuleUIBox>,
}

impl EditorState {
    pub fn new() -> Self {
        Self {
            selected_module_ui: None,
        }
    }
}

fn show_menu_item(ui: &mut Ui, label: &str, selected: bool) -> Response {
    let mut frame = Frame::NONE.inner_margin(Margin::symmetric(8, 4));

    if selected {
        frame = frame.fill(Color32::from_rgba_unmultiplied(255, 255, 255, 20));
    }

    let response = frame.show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.label(label)
    });

    ui.add(Separator::default().spacing(0.0));

    response.response.interact(Sense::click())
}

fn ui_for_module(module: &dyn SynthModule) -> ModuleUIBox {
    match module.module_type() {
        ModuleType::HarmonicEditor => Box::new(HarmonicEditorUI::new(module.id())),
        ModuleType::SpectralFilter => Box::new(SpectralFilterUI::new(module.id())),
        ModuleType::Amplifier => Box::new(AmplifierUI::new(module.id())),
        ModuleType::Oscillator => Box::new(OscillatorUI::new(module.id())),
        ModuleType::Envelope => Box::new(EnvelopeUI::new(module.id())),
        ModuleType::ExternalParam => Box::new(ExternalParamUI::new(module.id())),
        ModuleType::ModulationFilter => Box::new(ModulationFilterUI::new(module.id())),
        ModuleType::Lfo => Box::new(LfoUi::new(module.id())),
        ModuleType::SpectralBlend => Box::new(SpectralBlendUi::new(module.id())),
    }
}

fn show_side_bar(
    ui: &mut Ui,
    selected_module_ui: &mut Option<ModuleUIBox>,
    synth_engine: &mut SynthEngine,
) {
    SidePanel::left("side-bar")
        .resizable(true)
        .width_range(100.0..=200.0)
        .default_width(150.0)
        .frame(Frame::NONE)
        .show_inside(ui, |ui| {
            let mut modules = synth_engine.get_modules();

            modules.sort_by_key(|module| module.id());

            CentralPanel::default()
                .frame(Frame::NONE)
                .show_inside(ui, |ui| {
                    ScrollArea::vertical().show(ui, |ui| {
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);

                            if show_menu_item(ui, "Parameters", selected_module_ui.is_none())
                                .clicked()
                            {
                                *selected_module_ui = None;
                            }

                            for module in modules {
                                if show_menu_item(
                                    ui,
                                    &module.label(),
                                    selected_module_ui
                                        .as_ref()
                                        .is_some_and(|mod_ui| mod_ui.module_id() == module.id()),
                                )
                                .clicked()
                                {
                                    *selected_module_ui = Some(ui_for_module(module));
                                }
                            }
                        })
                    });
                });

            TopBottomPanel::bottom("side-bar-bottom")
                .resizable(false)
                .frame(Frame::new().inner_margin(8.0))
                .show_inside(ui, |ui| {
                    ui.vertical_centered_justified(|ui| {
                        ComboBox::from_id_salt("add-module-dropdown")
                            .selected_text("Add Module")
                            .width(ui.available_width())
                            .show_ui(ui, |ui| {
                                if ui.selectable_label(false, "Harmonic Editor").clicked() {
                                    synth_engine.add_harmonic_editor();
                                }
                                if ui.selectable_label(false, "Oscillator").clicked() {
                                    synth_engine.add_oscillator();
                                }
                                if ui.selectable_label(false, "Envelope").clicked() {
                                    synth_engine.add_envelope();
                                }
                                if ui.selectable_label(false, "LFO").clicked() {
                                    synth_engine.add_lfo();
                                }
                                if ui.selectable_label(false, "Spectral Filter").clicked() {
                                    synth_engine.add_spectral_filter();
                                }
                                if ui.selectable_label(false, "Spectral Blend").clicked() {
                                    synth_engine.add_spectral_blend();
                                }
                                if ui.selectable_label(false, "External Parameter").clicked() {
                                    synth_engine.add_external_param();
                                }
                                if ui.selectable_label(false, "Modulation Filter").clicked() {
                                    synth_engine.add_modulation_filter();
                                }
                                if ui.selectable_label(false, "Amplifier").clicked() {
                                    let amp_id = synth_engine.add_amplifier();

                                    synth_engine
                                        .add_link(
                                            amp_id,
                                            ModuleInput::new(Input::Audio, OUTPUT_MODULE_ID),
                                        )
                                        .unwrap();
                                }
                            });
                    });
                });
        });
}

impl VoiceOverride {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Kill => "Kill",
            Self::Steal => "Steal",
        }
    }
}

fn show_right_bar(ui: &mut Ui, synth_engine: &mut SynthEngine) {
    let mut level = synth_engine.get_output_level();

    SidePanel::right("right-bar")
        .exact_width(24.0)
        .resizable(false)
        .frame(Frame::new().inner_margin(vec2(4.0, 8.0)))
        .show_inside(ui, |ui| {
            if ui
                .add(
                    GainSlider::new(&mut level)
                        .width(16.0)
                        .max_dbs(6.0)
                        .label("Volume"),
                )
                .changed()
            {
                synth_engine.set_output_level(level);
            }
        });
}

fn show_params_ui(ui: &mut Ui, synth_engine: &mut SynthEngine) {
    ui.heading("Parameters");
    ui.add_space(20.0);

    Grid::new("params_grid")
        .num_columns(2)
        .spacing([40.0, 24.0])
        .striped(true)
        .show(ui, |ui| {
            let buffer_sizes = [16, 32, 64, 128];
            let mut voices = synth_engine.get_voices_num();
            let mut buffer_size = synth_engine.get_buffer_size();
            let mut voice_override = synth_engine.get_voice_override();

            ui.label("Voices");
            if ui.add(Slider::new(&mut voices, 1..=16)).changed() {
                synth_engine.set_num_voices(voices);
            }
            ui.end_row();

            let overrides = [VoiceOverride::Kill, VoiceOverride::Steal];

            ui.label("Voice override");
            ComboBox::from_id_salt("voice-override-select")
                .selected_text(voice_override.label())
                .show_ui(ui, |ui| {
                    for vo in &overrides {
                        if ui
                            .selectable_value(&mut voice_override, *vo, vo.label())
                            .clicked()
                        {
                            synth_engine.set_voice_override(*vo);
                        }
                    }
                });
            ui.end_row();

            ui.label("Buffer Size");
            ComboBox::from_id_salt("buff-size-select")
                .selected_text(format!("{} samples", buffer_size))
                .show_ui(ui, |ui| {
                    for sz in &buffer_sizes {
                        if ui
                            .selectable_value(&mut buffer_size, *sz, format!("{} samples", sz))
                            .clicked()
                        {
                            synth_engine.set_buffer_size(*sz);
                        }
                    }
                });
            ui.end_row();
        });
}

fn show_editor(ui: &mut Ui, editor_state: &mut EditorState, synth_engine: &mut SynthEngine) {
    if let Some(module_ui) = &editor_state.selected_module_ui
        && !synth_engine.has_module_id(module_ui.module_id())
    {
        editor_state.selected_module_ui = None;
    }

    show_side_bar(ui, &mut editor_state.selected_module_ui, synth_engine);
    show_right_bar(ui, synth_engine);

    CentralPanel::default()
        .frame(Frame::default().inner_margin(8.0))
        .show_inside(ui, |ui| {
            if let Some(module_ui) = &mut editor_state.selected_module_ui {
                ScrollArea::vertical()
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        module_ui.ui(synth_engine, ui);
                    });
            } else {
                show_params_ui(ui, synth_engine);
            }
        });
}

pub fn create_editor(
    egui_state: Arc<EguiState>,
    synth_engine: Arc<Mutex<SynthEngine>>,
) -> Option<Box<dyn Editor>> {
    create_egui_editor(
        Arc::clone(&egui_state),
        EditorState::new(),
        |_, _| {},
        move |egui_ctx, _setter, editor_state| {
            ResizableWindow::new("res-wind")
                .min_size(Vec2::new(640.0, 480.0))
                .show(egui_ctx, egui_state.as_ref(), |ui| {
                    show_editor(ui, editor_state, &mut synth_engine.lock());
                });
        },
    )
}
