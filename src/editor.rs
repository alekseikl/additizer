use std::sync::Arc;

use egui::{
    CentralPanel, Color32, ComboBox, Frame, Margin, Panel, Response, ScrollArea, Sense, Separator,
    Ui, Vec2, vec2,
};
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
mod modulation_input;
mod module_label;
mod modules_ui;
mod routing_grid;
mod stereo_slider;
mod utils;
mod waveform;

pub trait ModuleUi {
    fn module_id(&self) -> Option<ModuleId>;
    fn ui(&mut self, bridge: &mut UiBridge, ui: &mut Ui);
}

type ModuleUIBox = Box<dyn ModuleUi + Send>;

struct EditorState {
    engine_factory: Arc<EngineFactory>,
    ui_bridge: UiBridge,
    selected_module_ui: ModuleUIBox,
    routing_grid: routing_grid::RoutingGrid,
}

impl EditorState {
    pub fn new(engine_factory: Arc<EngineFactory>) -> Self {
        let bridge =
            UiBridge::create(engine_factory.get_engine(), engine_factory.get_ui_config()).unwrap();

        Self {
            engine_factory: engine_factory.clone(),
            ui_bridge: bridge,
            selected_module_ui: Box::new(ParamsUi::new(engine_factory)),
            routing_grid: Default::default(),
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

fn show_side_bar(
    ui: &mut Ui,
    selected_module_ui: &mut ModuleUIBox,
    bridge: &mut UiBridge,
    engine_factory: &Arc<EngineFactory>,
) {
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
                    });
                });

            let mut modules = bridge.get_modules();

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
                                *selected_module_ui =
                                    Box::new(ParamsUi::new(engine_factory.clone()));
                            }

                            for module in modules {
                                if show_menu_item(
                                    ui,
                                    &module.label,
                                    selected_module_id.is_some_and(|mod_id| mod_id == module.id),
                                )
                                .clicked()
                                {
                                    *selected_module_ui = module.module_type.ui(module.id);
                                }
                            }
                        })
                    });
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

        editor_state.selected_module_ui =
            Box::new(ParamsUi::new(editor_state.engine_factory.clone()));
        editor_state.routing_grid = Default::default();
    }

    let bridge = &mut editor_state.ui_bridge;

    bridge.update();

    if let Some(module_id) = editor_state.selected_module_ui.module_id()
        && !bridge.has_module_id(module_id)
    {
        editor_state.selected_module_ui =
            Box::new(ParamsUi::new(editor_state.engine_factory.clone()));
    }

    show_side_bar(
        ui,
        &mut editor_state.selected_module_ui,
        bridge,
        &editor_state.engine_factory,
    );
    show_right_bar(ui, bridge);

    CentralPanel::default()
        .frame(Frame::NONE)
        .show_inside(ui, |ui| {
            editor_state.routing_grid.show(bridge, ui);
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
