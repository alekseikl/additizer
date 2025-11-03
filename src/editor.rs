use std::sync::Arc;

use egui_baseview::egui::{
    CentralPanel, Color32, Frame, Margin, Response, ScrollArea, Sense, Separator, SidePanel,
    TopBottomPanel, Ui, Vec2, style::ScrollStyle, vec2,
};
use nih_plug::editor::Editor;
use parking_lot::Mutex;

use crate::{
    editor::{gain_slider::GainSlider, stereo_slider::StereoSlider},
    synth_engine::{
        HarmonicEditor, ModuleId, ModuleType, SpectralFilter, StereoSample, SynthEngine,
        SynthModule,
    },
};

use egui_integration::{ResizableWindow, create_egui_editor};

pub use egui_integration::EguiState;

mod egui_integration;
mod gain_slider;
mod stereo_slider;

struct HarmonicEditorState {
    level: StereoSample,
}

impl Default for HarmonicEditorState {
    fn default() -> Self {
        Self {
            level: StereoSample::splat(-0.1),
        }
    }
}

struct EditorState {
    selected_module_id: Option<ModuleId>,
    harmonic_editor: HarmonicEditorState,
}

impl EditorState {
    pub fn new() -> Self {
        Self {
            selected_module_id: None,
            harmonic_editor: HarmonicEditorState::default(),
        }
    }
}

fn show_menu_item(ui: &mut Ui, module: &dyn SynthModule, selected: bool) -> Response {
    let label = module.label();
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

fn show_side_bar(
    ui: &mut Ui,
    selected_module_id: &mut Option<ModuleId>,
    synth_engine: &mut SynthEngine,
) {
    let mut modules = synth_engine.get_modules();

    modules.sort_by_key(|module| module.id());

    if selected_module_id.is_none() && !modules.is_empty() {
        *selected_module_id = Some(modules[0].id());
    }

    SidePanel::left("side-bar")
        .resizable(true)
        .width_range(100.0..=200.0)
        .default_width(150.0)
        .frame(Frame::NONE)
        .show_inside(ui, |ui| {
            ScrollArea::vertical().show(ui, |ui| {
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing = vec2(0.0, 0.0);

                    for module in modules {
                        if show_menu_item(ui, module, *selected_module_id == Some(module.id()))
                            .clicked()
                        {
                            *selected_module_id = Some(module.id());
                        }
                    }
                })
            });
        });
}

fn harmonic_editor_ui(
    ui: &mut Ui,
    synth_engine: &mut SynthEngine,
    state: &mut HarmonicEditorState,
    module_id: ModuleId,
) {
    let harmonic_editor =
        HarmonicEditor::downcast_mut(synth_engine.get_module_mut(module_id).unwrap()).unwrap();
    let mut need_update = false;

    ui.style_mut().spacing.scroll = ScrollStyle::solid();

    TopBottomPanel::top("harmonics-list")
        .resizable(true)
        .height_range(150.0..=400.0)
        .default_height(200.0)
        .frame(Frame::NONE.inner_margin(Margin {
            left: 0,
            top: 0,
            right: 0,
            bottom: 8,
        }))
        .show_inside(ui, |ui| {
            ScrollArea::horizontal().show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    let harmonics = harmonic_editor.harmonics_ref_mut();
                    let height = ui.available_height();

                    ui.style_mut().spacing.item_spacing = Vec2::splat(2.0);
                    ui.style_mut().interaction.tooltip_delay = 0.1;
                    ui.style_mut().interaction.show_tooltips_only_when_still = false;

                    for (idx, harmonic) in harmonics.iter_mut().enumerate() {
                        if ui
                            .add(
                                GainSlider::new(harmonic)
                                    .label(&format!("{}", idx + 1))
                                    .height(height),
                            )
                            .changed()
                        {
                            need_update = true;
                        }
                    }
                });
            });
        });

    CentralPanel::default().show_inside(ui, |ui| {
        ui.label("Harmonics editor");
        ui.add(StereoSlider::level(&mut state.level));
        ui.add(StereoSlider::level_mod(&mut state.level).skew(1.8));
    });

    if need_update {
        harmonic_editor.apply_harmonics();
    }
}

fn spectral_filter_ui(ui: &mut Ui, synth_engine: &mut SynthEngine, module_id: ModuleId) {
    let spectral_filter =
        SpectralFilter::downcast_mut(synth_engine.get_module_mut(module_id).unwrap()).unwrap();

    let mut filter_ui = spectral_filter.get_ui();

    if ui
        .add(StereoSlider::octave(&mut filter_ui.cutoff).width(200.0))
        .changed()
    {
        spectral_filter.set_cutoff(filter_ui.cutoff);
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

fn show_editor(ui: &mut Ui, editor_state: &mut EditorState, synth_engine: &mut SynthEngine) {
    show_side_bar(ui, &mut editor_state.selected_module_id, synth_engine);
    show_right_bar(ui, synth_engine);

    CentralPanel::default()
        .frame(Frame::default().inner_margin(8.0))
        .show_inside(ui, |ui| {
            if let Some(module_id) = editor_state.selected_module_id
                && let Some(module) = synth_engine.get_module(module_id)
            {
                match module.module_type() {
                    ModuleType::HarmonicEditor => harmonic_editor_ui(
                        ui,
                        synth_engine,
                        &mut editor_state.harmonic_editor,
                        module_id,
                    ),
                    ModuleType::SpectralFilter => spectral_filter_ui(ui, synth_engine, module_id),
                    _ => (),
                }
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
