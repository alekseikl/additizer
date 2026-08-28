use crate::{
    editor::{
        ModuleUi,
        bin_slider::{BinSlider, BinSliderMode},
        module_label::ModuleLabel,
        slider::Slider,
        units::Units,
    },
    synth_engine::{
        ModuleId, ModuleType, SPECTRAL_BUFFER_SIZE, Sample, StereoSample,
        harmonic_editor::{EditRequest, HarmonicEditorUiBridge, sawtooth_phase},
        ui_bridge::{ModuleBridge, UiBridge},
    },
    utils::db_to_gain,
};
use egui::{ComboBox, DragValue, Grid, Id, Modal, ScrollArea, Sides, Ui, Vec2, style::ScrollStyle};

const MIN_HARMONIC: u16 = 1;
const MAX_HARMONIC: u16 = (SPECTRAL_BUFFER_SIZE - 1) as u16;
const MIN_GAIN_DB: Sample = -48.0;
const GAIN_DB_RANGE: std::ops::RangeInclusive<Sample> = MIN_GAIN_DB..=24.0;
const HARMONICS_HEIGHT: f32 = 160.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum EditKind {
    Range,
    NthElement,
    RandomAmplitudes,
}

impl EditKind {
    const ALL: &[Self] = &[Self::Range, Self::NthElement, Self::RandomAmplitudes];

    fn label(self) -> &'static str {
        match self {
            Self::Range => "Set range",
            Self::NthElement => "Set n-th",
            Self::RandomAmplitudes => "Randomize amplitudes",
        }
    }
}

struct EditFormState {
    kind: EditKind,
    harmonic_from: u16,
    harmonic_to: u16,
    mul: u8,
    add: u8,
    gain_db: StereoSample,
    level_from: Sample,
    level_to: Sample,
    stereo: Sample,
}

impl Default for EditFormState {
    fn default() -> Self {
        Self {
            kind: EditKind::Range,
            harmonic_from: MIN_HARMONIC,
            harmonic_to: MAX_HARMONIC,
            mul: 2,
            add: 1,
            gain_db: StereoSample::ZERO,
            level_from: -12.0,
            level_to: 0.0,
            stereo: 0.1,
        }
    }
}

impl EditFormState {
    fn to_request(&self) -> EditRequest {
        let gain = self.gain_db.map(|dbs| {
            if dbs <= MIN_GAIN_DB {
                0.0
            } else {
                db_to_gain(dbs)
            }
        });

        match self.kind {
            EditKind::Range => EditRequest::Range {
                harmonic_from: self.harmonic_from,
                harmonic_to: self.harmonic_to,
                gain,
            },
            EditKind::NthElement => EditRequest::NthElement {
                harmonic_from: self.harmonic_from,
                harmonic_to: self.harmonic_to,
                mul: self.mul,
                add: self.add,
                gain,
            },
            EditKind::RandomAmplitudes => EditRequest::RandomAmplitudes {
                level_from: self.level_from,
                level_to: self.level_to,
                stereo: self.stereo,
            },
        }
    }
}

pub struct HarmonicEditorUI {
    module_id: ModuleId,
    edit_form: Option<EditFormState>,
    bin_mode: BinSliderMode,
}

impl HarmonicEditorUI {
    pub fn new(module_id: ModuleId) -> Self {
        Self {
            module_id,
            edit_form: None,
            bin_mode: BinSliderMode::Amplitude,
        }
    }

    fn paint_ui(
        &mut self,
        bridge: &mut UiBridge,
        editor_bridge: &mut HarmonicEditorUiBridge,
        ui: &mut Ui,
    ) {
        let module_id = self.module_id;

        ui.add(ModuleLabel::new(
            module_id,
            ModuleType::HarmonicEditor,
            bridge,
        ));

        ui.add_space(16.0);

        ui.horizontal(|ui| {
            ComboBox::from_id_salt(("harmonic-bin-view", module_id))
                .selected_text(self.bin_mode.label())
                .width(0.0)
                .show_ui(ui, |ui| {
                    for &mode in BinSliderMode::ALL {
                        ui.selectable_value(&mut self.bin_mode, mode, mode.label());
                    }
                });
        });

        ui.add_space(8.0);

        ui.style_mut().spacing.scroll = ScrollStyle::solid();
        ui.allocate_ui(Vec2::new(ui.available_width(), HARMONICS_HEIGHT), |ui| {
            ScrollArea::horizontal()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.horizontal_top(|ui| {
                        let height = ui.available_height();

                        ui.style_mut().spacing.item_spacing = Vec2::splat(2.0);
                        ui.style_mut().interaction.tooltip_delay = 0.1;
                        ui.style_mut().interaction.show_tooltips_only_when_still = false;

                        let mut changed = None;
                        {
                            let harmonics = editor_bridge.harmonics_mut();

                            for idx in 1..SPECTRAL_BUFFER_SIZE {
                                let mut value = match self.bin_mode {
                                    BinSliderMode::Amplitude => harmonics.amplitude(idx),
                                    BinSliderMode::Phase => harmonics.phase(idx),
                                };

                                if ui
                                    .add(
                                        BinSlider::new(&mut value)
                                            .mode(self.bin_mode)
                                            .default(match self.bin_mode {
                                                BinSliderMode::Amplitude => {
                                                    StereoSample::splat(1.0)
                                                }
                                                BinSliderMode::Phase => {
                                                    StereoSample::splat(sawtooth_phase(idx))
                                                }
                                            })
                                            .label(&format!("{}", idx))
                                            .height(height),
                                    )
                                    .changed()
                                {
                                    match self.bin_mode {
                                        BinSliderMode::Amplitude => {
                                            harmonics.set_amplitude(idx, value);
                                        }
                                        BinSliderMode::Phase => {
                                            harmonics.set_phase(idx, value);
                                        }
                                    }
                                    changed = Some((idx, value));
                                }
                            }
                        }

                        if let Some((idx, value)) = changed {
                            match self.bin_mode {
                                BinSliderMode::Amplitude => {
                                    editor_bridge.set_harmonic(idx, value);
                                }
                                BinSliderMode::Phase => {
                                    editor_bridge.set_phase(idx, value);
                                }
                            }
                        }
                    });
                });
        });

        ui.add_space(16.0);

        ui.horizontal(|ui| {
            if ui.button("Clear").clicked() {
                editor_bridge.clear();
            }

            if ui.button("Reset Sawtooth").clicked() {
                editor_bridge.reset_sawtooth();
            }

            if ui.button("Edit").clicked() {
                let state = EditFormState::default();
                editor_bridge.edit_request(state.to_request());
                self.edit_form = Some(state);
            }
        });

        if let Some(mut state) = self.edit_form.take()
            && Self::show_edit_modal(editor_bridge, ui, self.module_id, &mut state)
        {
            self.edit_form.replace(state);
        }
    }

    fn show_edit_modal(
        editor_bridge: &mut HarmonicEditorUiBridge,
        ui: &mut Ui,
        module_id: ModuleId,
        state: &mut EditFormState,
    ) -> bool {
        let mut saved = false;
        let mut discarded = false;

        let modal =
            Modal::new(Id::new(("harmonic-editor-edit-modal", module_id))).show(ui.ctx(), |ui| {
                ui.heading("Edit Harmonics");
                ui.add_space(20.0);
                ui.set_width(440.0);

                let mut changed = false;

                Grid::new("harmonic-editor-edit-form")
                    .num_columns(2)
                    .spacing([40.0, 24.0])
                    .striped(true)
                    .show(ui, |ui| {
                        ui.label("Type");
                        ComboBox::from_id_salt("edit-request-kind")
                            .selected_text(state.kind.label())
                            .show_ui(ui, |ui| {
                                for &kind in EditKind::ALL {
                                    changed |= ui
                                        .selectable_value(&mut state.kind, kind, kind.label())
                                        .changed();
                                }
                            });
                        ui.end_row();

                        if state.kind != EditKind::RandomAmplitudes {
                            ui.label("Harmonics");
                            ui.horizontal(|ui| {
                                changed |= ui
                                    .add(
                                        DragValue::new(&mut state.harmonic_from)
                                            .range(MIN_HARMONIC..=MAX_HARMONIC),
                                    )
                                    .changed();
                                ui.label(" — ");
                                changed |= ui
                                    .add(
                                        DragValue::new(&mut state.harmonic_to)
                                            .range(MIN_HARMONIC..=MAX_HARMONIC),
                                    )
                                    .changed();
                            });
                            ui.end_row();
                        }

                        if state.kind == EditKind::NthElement {
                            ui.label("N-th Element");
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 0.0;
                                changed |= ui
                                    .add(DragValue::new(&mut state.mul).range(2..=50))
                                    .changed();
                                ui.label("n + ");
                                changed |= ui
                                    .add(
                                        DragValue::new(&mut state.add)
                                            .range(0..=state.mul.saturating_sub(1)),
                                    )
                                    .changed();
                            });
                            ui.end_row();
                        }

                        if state.kind == EditKind::RandomAmplitudes {
                            ui.label("From");
                            changed |= ui
                                .add(
                                    Slider::mono(&mut state.level_from, GAIN_DB_RANGE, None)
                                        .over(0.0)
                                        .units(Units::Db)
                                        .default(-12.0),
                                )
                                .changed();
                            ui.end_row();

                            ui.label("To");
                            changed |= ui
                                .add(
                                    Slider::mono(&mut state.level_to, GAIN_DB_RANGE, None)
                                        .over(0.0)
                                        .units(Units::Db)
                                        .default(0.0),
                                )
                                .changed();
                            ui.end_row();

                            ui.label("Stereo");
                            changed |= ui
                                .add(
                                    Slider::mono(&mut state.stereo, 0.0..=1.0, None).default(0.0),
                                )
                                .changed();
                            ui.end_row();
                        } else {
                            ui.label("Gain");
                            changed |= ui
                                .add(
                                    Slider::stereo(&mut state.gain_db, GAIN_DB_RANGE, None)
                                        .over(0.0)
                                        .units(Units::Db)
                                        .default(0.0),
                                )
                                .changed();
                            ui.end_row();
                        }
                    });

                if changed {
                    editor_bridge.edit_request(state.to_request());
                }

                ui.add_space(40.0);

                Sides::new().show(
                    ui,
                    |_ui| {},
                    |ui| {
                        if ui.button("Ok").clicked() {
                            editor_bridge.apply_draft();
                            saved = true;
                            ui.close();
                        }

                        if ui.button("Apply").clicked() {
                            editor_bridge.apply_draft();
                        }

                        if ui.button("Close").clicked() {
                            editor_bridge.discard_draft();
                            discarded = true;
                            ui.close();
                        }
                    },
                );
            });

        if modal.should_close() {
            if !saved && !discarded {
                editor_bridge.discard_draft();
            }
            false
        } else {
            true
        }
    }
}

impl ModuleUi for HarmonicEditorUI {
    fn module_id(&self) -> Option<ModuleId> {
        Some(self.module_id)
    }

    fn ui(&mut self, bridge: &mut UiBridge, ui: &mut Ui) {
        bridge.with_module_bridge(self.module_id, |bridge, module_bridge| {
            if let ModuleBridge::HarmonicEditor(editor_bridge) = module_bridge {
                self.paint_ui(bridge, editor_bridge, ui);
            }
        });
    }
}
