use std::sync::Arc;

use egui_baseview::egui::{
    CentralPanel, Color32, ComboBox, Frame, Margin, Response, ScrollArea, Sense, Separator,
    SidePanel, TopBottomPanel, Ui, Vec2, vec2,
};
use nih_plug::editor::Editor;
use parking_lot::Mutex;

use crate::{
    editor::{
        gain_slider::GainSlider,
        modules_ui::{
            AmplifierUI, EnvelopeUI, ExpressionsUi, ExternalParamUI, HarmonicEditorUI, LfoUi,
            MixerUi, ModulationFilterUI, OscillatorUI, ParamsUi, SpectralBlendUi, SpectralFilterUI,
            SpectralMixerUi, WaveShaperUi,
        },
    },
    synth_engine::{ModuleId, ModuleType, SynthEngine},
};

use egui_integration::{ResizableWindow, create_egui_editor};

pub use egui_integration::EguiState;

mod db_slider;
mod direct_input;
mod egui_integration;
mod gain_slider;
mod modulation_input;
mod module_label;
mod modules_ui;
mod multi_input;
mod stereo_slider;
mod utils;

pub trait ModuleUi {
    fn module_id(&self) -> Option<ModuleId>;
    fn ui(&mut self, synth: &mut SynthEngine, ui: &mut Ui);
}

type ModuleUIBox = Box<dyn ModuleUi + Send + Sync>;

struct EditorState {
    selected_module_ui: ModuleUIBox,
}

impl EditorState {
    pub fn new() -> Self {
        Self {
            selected_module_ui: Box::new(ParamsUi::new()),
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

impl ModuleType {
    fn ui(&self, id: ModuleId) -> ModuleUIBox {
        match self {
            Self::HarmonicEditor => Box::new(HarmonicEditorUI::new(id)),
            Self::SpectralFilter => Box::new(SpectralFilterUI::new(id)),
            Self::Amplifier => Box::new(AmplifierUI::new(id)),
            Self::Mixer => Box::new(MixerUi::new(id)),
            Self::Oscillator => Box::new(OscillatorUI::new(id)),
            Self::Envelope => Box::new(EnvelopeUI::new(id)),
            Self::ExternalParam => Box::new(ExternalParamUI::new(id)),
            Self::ModulationFilter => Box::new(ModulationFilterUI::new(id)),
            Self::Lfo => Box::new(LfoUi::new(id)),
            Self::SpectralBlend => Box::new(SpectralBlendUi::new(id)),
            Self::SpectralMixer => Box::new(SpectralMixerUi::new(id)),
            Self::WaveShaper => Box::new(WaveShaperUi::new(id)),
            Self::Expressions => Box::new(ExpressionsUi::new(id)),
        }
    }
}

fn show_side_bar(
    ui: &mut Ui,
    selected_module_ui: &mut ModuleUIBox,
    synth_engine: &mut SynthEngine,
) {
    SidePanel::left("side-bar")
        .resizable(true)
        .width_range(100.0..=200.0)
        .default_width(150.0)
        .frame(Frame::NONE)
        .show_inside(ui, |ui| {
            let mut modules = synth_engine.get_modules();

            modules.sort_by_key(|module| module.label().to_lowercase());

            CentralPanel::default()
                .frame(Frame::NONE)
                .show_inside(ui, |ui| {
                    ScrollArea::vertical().show(ui, |ui| {
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);

                            let selected_module_id = selected_module_ui.module_id();

                            if show_menu_item(ui, "Parameters", selected_module_id.is_none())
                                .clicked()
                            {
                                *selected_module_ui = Box::new(ParamsUi::new());
                            }

                            for module in modules {
                                if show_menu_item(
                                    ui,
                                    &module.label(),
                                    selected_module_id.is_some_and(|mod_id| mod_id == module.id()),
                                )
                                .clicked()
                                {
                                    *selected_module_ui = module.module_type().ui(module.id());
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
                                if ui.selectable_label(false, "Spectral Mixer").clicked() {
                                    synth_engine.add_spectral_mixer();
                                }
                                if ui.selectable_label(false, "External Parameter").clicked() {
                                    synth_engine.add_external_param();
                                }
                                if ui.selectable_label(false, "Expressions").clicked() {
                                    synth_engine.add_expressions();
                                }
                                if ui.selectable_label(false, "Modulation Filter").clicked() {
                                    synth_engine.add_modulation_filter();
                                }
                                if ui.selectable_label(false, "Waveshaper").clicked() {
                                    synth_engine.add_wave_shaper();
                                }
                                if ui.selectable_label(false, "Amplifier").clicked() {
                                    synth_engine.add_amplifier();
                                }
                                if ui.selectable_label(false, "Mixer").clicked() {
                                    synth_engine.add_mixer();
                                }
                            });
                    });
                });
        });
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
                        .mid_point(0.8)
                        .label("Volume"),
                )
                .changed()
            {
                synth_engine.set_output_level(level);
            }
        });
}

fn show_editor(ui: &mut Ui, editor_state: &mut EditorState, synth_engine: &mut SynthEngine) {
    if let Some(module_id) = editor_state.selected_module_ui.module_id()
        && !synth_engine.has_module_id(module_id)
    {
        editor_state.selected_module_ui = Box::new(ParamsUi::new());
    }

    show_side_bar(ui, &mut editor_state.selected_module_ui, synth_engine);
    show_right_bar(ui, synth_engine);

    CentralPanel::default()
        .frame(Frame::default().inner_margin(8.0))
        .show_inside(ui, |ui| {
            ScrollArea::vertical()
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    editor_state.selected_module_ui.ui(synth_engine, ui);
                });
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
