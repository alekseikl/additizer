use std::sync::Arc;

use egui_baseview::egui::{
    CentralPanel, Checkbox, Color32, Frame, Grid, Margin, Response, ScrollArea, Sense, Separator,
    SidePanel, Ui, Vec2, vec2,
};
use nih_plug::editor::Editor;
use parking_lot::Mutex;

use crate::{
    editor::{
        gain_slider::GainSlider,
        modules_ui::{AmplifierUI, HarmonicEditorUI, OscillatorUI, SpectralFilterUI},
        stereo_slider::StereoSlider,
    },
    synth_engine::{Envelope, ModuleId, ModuleType, SynthEngine, SynthModule},
    utils::from_ms,
};

use egui_integration::{ResizableWindow, create_egui_editor};

pub use egui_integration::EguiState;

mod direct_input;
mod egui_integration;
mod gain_slider;
mod modulation_input;
mod modules_ui;
mod stereo_slider;

struct EditorState {
    selected_module_id: Option<ModuleId>,
}

impl EditorState {
    pub fn new() -> Self {
        Self {
            selected_module_id: None,
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

fn envelope_ui(ui: &mut Ui, synth_engine: &mut SynthEngine, module_id: ModuleId) {
    let env = Envelope::downcast_mut_unwrap(synth_engine.get_module_mut(module_id));
    let mut env_ui = env.get_ui();

    ui.heading("Envelope");

    Grid::new("env_grid")
        .num_columns(2)
        .spacing([40.0, 4.0])
        .striped(true)
        .show(ui, |ui| {
            ui.label("Attack");
            if ui
                .add(StereoSlider::envelope_time(&mut env_ui.attack).default_value(from_ms(1.0)))
                .changed()
            {
                env.set_attack(env_ui.attack);
            }
            ui.end_row();

            ui.label("Decay");
            if ui
                .add(StereoSlider::envelope_time(&mut env_ui.decay).default_value(from_ms(100.0)))
                .changed()
            {
                env.set_decay(env_ui.decay);
            }
            ui.end_row();

            ui.label("Sustain");
            if ui
                .add(StereoSlider::level(&mut env_ui.sustain).default_value(0.5))
                .changed()
            {
                env.set_sustain(env_ui.sustain);
            }
            ui.end_row();

            ui.label("Release");
            if ui
                .add(StereoSlider::envelope_time(&mut env_ui.release).default_value(from_ms(100.0)))
                .changed()
            {
                env.set_release(env_ui.release);
            }
            ui.end_row();

            ui.label("Keep voice alive");
            if ui
                .add(Checkbox::without_text(&mut env_ui.keep_voice_alive))
                .changed()
            {
                env.set_keep_voice_alive(env_ui.keep_voice_alive);
            }
            ui.end_row();
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
                    ModuleType::HarmonicEditor => {
                        ui.add(HarmonicEditorUI::new(module_id, synth_engine));
                    }
                    ModuleType::SpectralFilter => {
                        ui.add(SpectralFilterUI::new(module_id, synth_engine));
                    }
                    ModuleType::Amplifier => {
                        ui.add(AmplifierUI::new(module_id, synth_engine));
                    }
                    ModuleType::Oscillator => {
                        ui.add(OscillatorUI::new(module_id, synth_engine));
                    }
                    ModuleType::Envelope => envelope_ui(ui, synth_engine, module_id),
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
