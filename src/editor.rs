use std::sync::Arc;

use egui::{
    CentralPanel, Color32, ComboBox, Frame, Margin, Panel, Response, ScrollArea, Sense, Separator,
    Ui, Vec2, vec2,
};
use nih_plug::editor::Editor;
use nih_plug_egui::{EguiState, create_egui_editor, resizable_window::ResizableWindow};
use parking_lot::Mutex;

use crate::{
    editor::{
        gain_slider::GainSlider,
        modules_ui::{
            AmplifierUI, EnvelopeUI, ExpressionsUi, ExternalParamUI, HarmonicEditorUI, LfoUi,
            MixerUi, OscillatorUI, ParamsUi, SpectralBlendUi, SpectralFilterUI, SpectralMixerUi,
            WaveShaperUi,
        },
    },
    synth_engine::{ModuleId, ModuleType, OUTPUT_MODULE_ID, SynthEngine, ui_bridge::UiBridge},
};

pub type SynthEngineHandle = Arc<Mutex<SynthEngine>>;

mod db_slider;
mod direct_input;
mod gain_slider;
mod modulation_input;
mod module_label;
mod modules_ui;
mod multi_input;
mod stereo_slider;
mod utils;

pub trait ModuleUi {
    fn module_id(&self) -> Option<ModuleId>;
    fn ui(&mut self, bridge: &mut UiBridge, ui: &mut Ui);
    fn cleanup(&mut self, _bridge: &mut UiBridge) {}
}

type ModuleUIBox = Box<dyn ModuleUi + Send>;

struct EditorState {
    synth_bridge: UiBridge,
    selected_module_ui: ModuleUIBox,
}

impl EditorState {
    pub fn new(synth_bridge: UiBridge) -> Self {
        Self {
            synth_bridge,
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

struct OutputUi;

impl ModuleUi for OutputUi {
    fn module_id(&self) -> Option<ModuleId> {
        Some(OUTPUT_MODULE_ID)
    }

    fn ui(&mut self, _bridge: &mut UiBridge, _ui: &mut Ui) {}
}

impl ModuleType {
    fn ui(&self, id: ModuleId, synth_engine: &SynthEngineHandle) -> ModuleUIBox {
        match self {
            Self::Output => Box::new(OutputUi),
            Self::HarmonicEditor => Box::new(HarmonicEditorUI::new(id)),
            Self::SpectralFilter => Box::new(SpectralFilterUI::new(id)),
            Self::Amplifier => Box::new(AmplifierUI::new(id)),
            Self::Mixer => Box::new(MixerUi::new(id)),
            Self::Oscillator => Box::new(OscillatorUI::new(id, synth_engine)),
            Self::Envelope => Box::new(EnvelopeUI::new(id)),
            Self::ExternalParam => Box::new(ExternalParamUI::new(id)),
            Self::Lfo => Box::new(LfoUi::new(id)),
            Self::SpectralBlend => Box::new(SpectralBlendUi::new(id)),
            Self::SpectralMixer => Box::new(SpectralMixerUi::new(id)),
            Self::WaveShaper => Box::new(WaveShaperUi::new(id)),
            Self::Expressions => Box::new(ExpressionsUi::new(id)),
        }
    }
}

fn show_side_bar(ui: &mut Ui, selected_module_ui: &mut ModuleUIBox, bridge: &mut UiBridge) {
    let synth_engine = bridge.synth().clone();
    Panel::left("side-bar")
        .resizable(true)
        .size_range(100.0..=200.0)
        .default_size(150.0)
        .frame(Frame::NONE)
        .show_inside(ui, |ui| {
            Panel::bottom("side-bar-bottom")
                .resizable(false)
                .frame(Frame::new().inner_margin(8.0))
                .show_inside(ui, |ui| {
                    ui.vertical_centered_justified(|ui| {
                        ComboBox::from_id_salt("add-module-dropdown")
                            .selected_text("Add Module")
                            .width(ui.available_width())
                            .show_ui(ui, |ui| {
                                if ui.selectable_label(false, "Harmonic Editor").clicked() {
                                    synth_engine.lock().add_harmonic_editor();
                                }
                                if ui.selectable_label(false, "Oscillator").clicked() {
                                    synth_engine.lock().add_oscillator();
                                }
                                if ui.selectable_label(false, "Envelope").clicked() {
                                    synth_engine.lock().add_envelope();
                                }
                                if ui.selectable_label(false, "LFO").clicked() {
                                    synth_engine.lock().add_lfo();
                                }
                                if ui.selectable_label(false, "Spectral Filter").clicked() {
                                    synth_engine.lock().add_spectral_filter();
                                }
                                if ui.selectable_label(false, "Spectral Blend").clicked() {
                                    synth_engine.lock().add_spectral_blend();
                                }
                                if ui.selectable_label(false, "Spectral Mixer").clicked() {
                                    synth_engine.lock().add_spectral_mixer();
                                }
                                if ui.selectable_label(false, "External Parameter").clicked() {
                                    synth_engine.lock().add_external_param();
                                }
                                if ui.selectable_label(false, "Expressions").clicked() {
                                    synth_engine.lock().add_expressions();
                                }
                                if ui.selectable_label(false, "Waveshaper").clicked() {
                                    synth_engine.lock().add_wave_shaper();
                                }
                                if ui.selectable_label(false, "Amplifier").clicked() {
                                    synth_engine.lock().add_amplifier();
                                }
                                if ui.selectable_label(false, "Mixer").clicked() {
                                    synth_engine.lock().add_mixer();
                                }
                            });
                    });
                });

            let mut modules = synth_engine.lock().get_routing_state().get_modules();

            modules.sort_by_key(|module| module.label.to_lowercase());

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
                                selected_module_ui.cleanup(bridge);
                                *selected_module_ui = Box::new(ParamsUi::new());
                            }

                            for module in modules {
                                if show_menu_item(
                                    ui,
                                    &module.label,
                                    selected_module_id.is_some_and(|mod_id| mod_id == module.id),
                                )
                                .clicked()
                                {
                                    selected_module_ui.cleanup(bridge);

                                    *selected_module_ui =
                                        module.module_type.ui(module.id, &synth_engine);
                                }
                            }
                        })
                    });
                });
        });
}

fn show_right_bar(ui: &mut Ui, bridge: &mut UiBridge) {
    let mut level = bridge.synth().lock().get_output_level();

    Panel::right("right-bar")
        .exact_size(24.0)
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
                bridge.synth().lock().set_output_level(level);
            }
        });
}

fn show_editor(ui: &mut Ui, editor_state: &mut EditorState) {
    editor_state.synth_bridge.update();

    let bridge = &mut editor_state.synth_bridge;

    bridge.update();

    if let Some(module_id) = editor_state.selected_module_ui.module_id()
        && !bridge.synth().lock().has_module_id(module_id)
    {
        editor_state.selected_module_ui = Box::new(ParamsUi::new());
    }

    show_side_bar(ui, &mut editor_state.selected_module_ui, bridge);
    show_right_bar(ui, bridge);

    CentralPanel::default()
        .frame(Frame::default().inner_margin(8.0))
        .show_inside(ui, |ui| {
            ScrollArea::vertical()
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    editor_state.selected_module_ui.ui(bridge, ui);
                });
        });
}

pub fn create_editor(
    egui_state: Arc<EguiState>,
    synth_engine: SynthEngineHandle,
) -> Option<Box<dyn Editor>> {
    create_egui_editor(
        Arc::clone(&egui_state),
        EditorState::new(UiBridge::create(synth_engine.clone()).unwrap()),
        Default::default(),
        |_egui_ctx, _queue, editor_state| {
            editor_state.synth_bridge.sync();
        },
        move |egui_ctx, _setter, _queue, editor_state| {
            ResizableWindow::new("res-wind")
                .min_size(Vec2::new(640.0, 480.0))
                .show(egui_ctx, egui_state.as_ref(), |ui| {
                    show_editor(ui, editor_state);
                });
        },
    )
}
