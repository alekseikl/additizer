use std::sync::Arc;

use egui_baseview::egui::{
    CentralPanel, Checkbox, Color32, DragValue, Frame, Grid, Margin, Response, ScrollArea, Sense,
    Separator, SidePanel, TopBottomPanel, Ui, Vec2, style::ScrollStyle, vec2,
};
use nih_plug::editor::Editor;
use parking_lot::Mutex;

use crate::{
    editor::{gain_slider::GainSlider, stereo_slider::StereoSlider},
    synth_engine::{
        Amplifier, Envelope, HarmonicEditor, ModuleId, ModuleType, Oscillator, SpectralFilter,
        StereoSample, SynthEngine, SynthModule,
    },
    utils::{from_ms, st_to_octave},
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
    _state: &mut HarmonicEditorState,
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
    });

    if need_update {
        harmonic_editor.apply_harmonics();
    }
}

fn spectral_filter_ui(ui: &mut Ui, synth_engine: &mut SynthEngine, module_id: ModuleId) {
    let spectral_filter =
        SpectralFilter::downcast_mut_unwrap(synth_engine.get_module_mut(module_id));

    let mut filter_ui = spectral_filter.get_ui();

    ui.heading("Spectral filter");

    Grid::new("sf_grid")
        .num_columns(2)
        .spacing([40.0, 4.0])
        .striped(true)
        .show(ui, |ui| {
            ui.label("Cutoff");
            if ui
                .add(StereoSlider::octave(&mut filter_ui.cutoff).width(200.0))
                .changed()
            {
                spectral_filter.set_cutoff(filter_ui.cutoff);
            }
            ui.end_row();

            ui.label("Q");
            if ui
                .add(StereoSlider::q(&mut filter_ui.q).width(200.0))
                .changed()
            {
                spectral_filter.set_q(filter_ui.q);
            }
            ui.end_row();

            ui.label("Four pole");
            if ui
                .add(Checkbox::without_text(&mut filter_ui.four_pole))
                .changed()
            {
                spectral_filter.set_four_pole(filter_ui.four_pole);
            }
            ui.end_row();
        });
}

fn amplifier_ui(ui: &mut Ui, synth_engine: &mut SynthEngine, module_id: ModuleId) {
    let amp = Amplifier::downcast_mut_unwrap(synth_engine.get_module_mut(module_id));
    let mut amp_ui = amp.get_ui();

    ui.heading("Amplifier");

    Grid::new("amp_grid")
        .num_columns(2)
        .spacing([40.0, 4.0])
        .striped(true)
        .show(ui, |ui| {
            ui.label("Level");
            if ui
                .add(StereoSlider::level(&mut amp_ui.level).width(200.0))
                .changed()
            {
                amp.set_level(amp_ui.level);
            }
            ui.end_row();
        });
}

fn oscillator_ui(ui: &mut Ui, synth_engine: &mut SynthEngine, module_id: ModuleId) {
    let osc = Oscillator::downcast_mut_unwrap(synth_engine.get_module_mut(module_id));
    let mut osc_ui = osc.get_ui();

    ui.heading("Oscillator");

    Grid::new("osc_grid")
        .num_columns(2)
        .spacing([40.0, 4.0])
        .striped(true)
        .show(ui, |ui| {
            ui.label("Level");
            if ui.add(StereoSlider::level(&mut osc_ui.level)).changed() {
                osc.set_level(osc_ui.level);
            }
            ui.end_row();

            ui.label("Pitch shift");
            if ui
                .add(
                    StereoSlider::octave(&mut osc_ui.pitch_shift)
                        .range(0.0..=st_to_octave(60.0))
                        .skew(1.6)
                        .allow_inverse(),
                )
                .changed()
            {
                osc.set_pitch_shift(osc_ui.pitch_shift);
            }
            ui.end_row();

            ui.label("Detune");
            if ui
                .add(
                    StereoSlider::new(&mut osc_ui.detune)
                        .range(0.0..=st_to_octave(1.0))
                        .display_scale(1200.0)
                        .default_value(st_to_octave(0.2))
                        .units("cents"),
                )
                .changed()
            {
                osc.set_detune(osc_ui.detune);
            }
            ui.end_row();

            ui.label("Unison");
            if ui
                .add(DragValue::new(&mut osc_ui.unison).range(1..=16))
                .changed()
            {
                osc.set_unison(osc_ui.unison);
            }
            ui.end_row();

            ui.label("Same note phases");
            if ui
                .add(Checkbox::without_text(&mut osc_ui.same_channel_phases))
                .changed()
            {
                osc.set_same_channels_phases(osc_ui.same_channel_phases);
            }
            ui.end_row();
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
                    ModuleType::HarmonicEditor => harmonic_editor_ui(
                        ui,
                        synth_engine,
                        &mut editor_state.harmonic_editor,
                        module_id,
                    ),
                    ModuleType::SpectralFilter => spectral_filter_ui(ui, synth_engine, module_id),
                    ModuleType::Amplifier => amplifier_ui(ui, synth_engine, module_id),
                    ModuleType::Oscillator => oscillator_ui(ui, synth_engine, module_id),
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
