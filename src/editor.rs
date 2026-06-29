use std::sync::Arc;

use egui::{CentralPanel, Color32, ComboBox, Frame, Panel, ScrollArea, Stroke, Ui, Vec2, vec2};
use nih_plug::editor::Editor;
use nih_plug_egui::{EguiState, create_egui_editor, resizable_window::ResizableWindow};

use crate::{
    editor::{
        gain_slider::GainSlider,
        modules_ui::{
            AmplifierUI, EnvelopeUI, ExpressionsUi, ExternalParamUI, HarmonicEditorUI, LfoUi,
            MixerUi, OscillatorUI, ParamsUi, SpectralBlendUi, SpectralFilterUI, SpectralMixerUi,
            WaveShaperUi,
        },
    },
    engine_factory::EngineFactory,
    synth_engine::{ModuleId, ModuleType, OUTPUT_MODULE_ID, ui_bridge::UiBridge},
};

mod db_slider;
mod direct_input;
mod gain_slider;
mod grid;
mod modulation_input;
mod module_label;
mod modules_ui;
mod stereo_slider;
mod utils;
mod waveform;

pub trait ModuleUi {
    fn module_id(&self) -> Option<ModuleId>;
    fn ui(&mut self, bridge: &mut UiBridge, ui: &mut Ui);
}

type ModuleUIBox = Box<dyn ModuleUi + Send>;

const DETAIL_SEPARATOR_T: f32 = 3.0;
const C_DETAIL_SEPARATOR: Color32 = Color32::from_rgb(100, 102, 118);

struct EditorState {
    engine_factory: Arc<EngineFactory>,
    ui_bridge: UiBridge,
    grid_module_ui: Option<ModuleUIBox>,
    grid: grid::Grid,
}

impl EditorState {
    pub fn new(engine_factory: Arc<EngineFactory>) -> Self {
        let bridge =
            UiBridge::create(engine_factory.get_engine(), engine_factory.get_ui_config()).unwrap();

        Self {
            engine_factory: engine_factory.clone(),
            ui_bridge: bridge,
            grid_module_ui: None,
            grid: grid::Grid::new(),
        }
    }
}

struct OutputUi;

impl ModuleUi for OutputUi {
    fn module_id(&self) -> Option<ModuleId> {
        Some(OUTPUT_MODULE_ID)
    }

    fn ui(&mut self, _bridge: &mut UiBridge, _ui: &mut Ui) {}
}

impl ModuleType {
    fn ui(&self, id: ModuleId) -> ModuleUIBox {
        match self {
            Self::Output => Box::new(OutputUi),
            Self::HarmonicEditor => Box::new(HarmonicEditorUI::new(id)),
            Self::SpectralFilter => Box::new(SpectralFilterUI::new(id)),
            Self::Amplifier => Box::new(AmplifierUI::new(id)),
            Self::Mixer => Box::new(MixerUi::new(id)),
            Self::Oscillator => Box::new(OscillatorUI::new(id)),
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

fn module_ui_for_id(bridge: &UiBridge, id: ModuleId) -> Option<ModuleUIBox> {
    bridge
        .get_modules()
        .into_iter()
        .find(|module| module.id == id)
        .map(|module| module.module_type.ui(module.id))
}

fn show_add_module_menu(ui: &mut Ui, bridge: &mut UiBridge) {
    ComboBox::from_id_salt("add-module-dropdown")
        .selected_text("Add Module")
        .show_ui(ui, |ui| {
            if ui.selectable_label(false, "Harmonic Editor").clicked() {
                bridge.add_module(ModuleType::HarmonicEditor);
            }
            if ui.selectable_label(false, "Oscillator").clicked() {
                bridge.add_module(ModuleType::Oscillator);
            }
            if ui.selectable_label(false, "Envelope").clicked() {
                bridge.add_module(ModuleType::Envelope);
            }
            if ui.selectable_label(false, "LFO").clicked() {
                bridge.add_module(ModuleType::Lfo);
            }
            if ui.selectable_label(false, "Spectral Filter").clicked() {
                bridge.add_module(ModuleType::SpectralFilter);
            }
            if ui.selectable_label(false, "Spectral Blend").clicked() {
                bridge.add_module(ModuleType::SpectralBlend);
            }
            if ui.selectable_label(false, "Spectral Mixer").clicked() {
                bridge.add_module(ModuleType::SpectralMixer);
            }
            if ui.selectable_label(false, "External Parameter").clicked() {
                bridge.add_module(ModuleType::ExternalParam);
            }
            if ui.selectable_label(false, "Expressions").clicked() {
                bridge.add_module(ModuleType::Expressions);
            }
            if ui.selectable_label(false, "Waveshaper").clicked() {
                bridge.add_module(ModuleType::WaveShaper);
            }
            if ui.selectable_label(false, "Amplifier").clicked() {
                bridge.add_module(ModuleType::Amplifier);
            }
            if ui.selectable_label(false, "Mixer").clicked() {
                bridge.add_module(ModuleType::Mixer);
            }
        });
}

fn show_top_bar(
    ui: &mut Ui,
    bridge: &mut UiBridge,
    engine_factory: &Arc<EngineFactory>,
    grid_module_ui: &mut Option<ModuleUIBox>,
) {
    Panel::top("top-bar")
        .frame(Frame::new().inner_margin(vec2(8.0, 4.0)))
        .show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                let showing_params = grid_module_ui
                    .as_ref()
                    .is_some_and(|panel| panel.module_id().is_none());

                if ui.selectable_label(showing_params, "Parameters").clicked() {
                    if showing_params {
                        *grid_module_ui = None;
                    } else {
                        *grid_module_ui = Some(Box::new(ParamsUi::new(engine_factory.clone())));
                    }
                }

                show_add_module_menu(ui, bridge);
            });
        });
}

fn show_right_bar(ui: &mut Ui, bridge: &mut UiBridge) {
    let mut level = bridge.engine_params().output_gain;

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
                bridge.set_output_gain(level);
            }
        });
}

fn show_editor(ui: &mut Ui, editor_state: &mut EditorState) {
    if editor_state
        .engine_factory
        .engine_changed(editor_state.ui_bridge.engine())
    {
        editor_state.ui_bridge = UiBridge::create(
            editor_state.engine_factory.get_engine(),
            editor_state.engine_factory.get_ui_config(),
        )
        .unwrap();

        editor_state.grid_module_ui = None;
        editor_state.grid = grid::Grid::new();
    }

    let bridge = &mut editor_state.ui_bridge;

    bridge.update();

    if let Some(module_id) = editor_state
        .grid_module_ui
        .as_ref()
        .and_then(|panel| panel.module_id())
        && !bridge.has_module_id(module_id)
    {
        editor_state.grid_module_ui = None;
    }

    show_top_bar(
        ui,
        bridge,
        &editor_state.engine_factory,
        &mut editor_state.grid_module_ui,
    );
    show_right_bar(ui, bridge);

    if let Some(modules_io) = bridge.take_modules_io() {
        editor_state.grid.update_widgets(modules_io);
    }

    CentralPanel::default()
        .frame(Frame::NONE)
        .show_inside(ui, |ui| {
            let grid_selected_id = editor_state
                .grid_module_ui
                .as_ref()
                .and_then(|panel| panel.module_id());

            let detail_rect = if editor_state.grid_module_ui.is_some() {
                let detail = Panel::bottom("grid-module-detail")
                    .resizable(true)
                    .default_size(300.0)
                    .min_size(80.0)
                    .frame(Frame::default().inner_margin(8.0))
                    .show_inside(ui, |ui| {
                        ScrollArea::vertical()
                            .auto_shrink([false, true])
                            .show(ui, |ui| {
                                if let Some(module_ui) = &mut editor_state.grid_module_ui {
                                    module_ui.ui(&mut editor_state.ui_bridge, ui);
                                }
                            });
                    });

                Some(detail.response.rect)
            } else {
                None
            };

            let opened = {
                let bridge = &mut editor_state.ui_bridge;
                editor_state.grid.ui(ui, bridge, grid_selected_id)
            };

            if let Some(id) = opened {
                editor_state.grid_module_ui = module_ui_for_id(&editor_state.ui_bridge, id);
            }

            if let Some(rect) = detail_rect {
                ui.painter().hline(
                    rect.left()..=rect.right(),
                    rect.top() + DETAIL_SEPARATOR_T * 0.5,
                    Stroke::new(DETAIL_SEPARATOR_T, C_DETAIL_SEPARATOR),
                );
            }
        });
}

pub fn create_editor(
    egui_state: Arc<EguiState>,
    factory: Arc<EngineFactory>,
) -> Option<Box<dyn Editor>> {
    create_egui_editor(
        Arc::clone(&egui_state),
        EditorState::new(factory),
        Default::default(),
        |_egui_ctx, _queue, _editor_state| {},
        move |egui_ctx, _setter, _queue, editor_state| {
            ResizableWindow::new("res-wind")
                .min_size(Vec2::new(640.0, 480.0))
                .show(egui_ctx, egui_state.as_ref(), |ui| {
                    show_editor(ui, editor_state);
                });
        },
    )
}
