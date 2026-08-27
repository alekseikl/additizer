use crate::{
    editor::{
        ModuleUi, gain_slider::GainSlider, module_label::ModuleLabel, slider::Slider, units::Units,
    },
    synth_engine::{
        ModuleId, ModuleType, SPECTRAL_BUFFER_SIZE, Sample, StereoSample,
        harmonic_editor::{EditRequest, HarmonicEditorUiBridge},
        ui_bridge::{ModuleBridge, UiBridge},
    },
    utils::db_to_gain,
};
use egui::{
    ComboBox, DragValue, Frame, Grid, Id, Margin, Modal, Panel, ScrollArea, Sides, Ui, Vec2,
    style::ScrollStyle,
};

const MIN_HARMONIC: u16 = 1;
const MAX_HARMONIC: u16 = (SPECTRAL_BUFFER_SIZE - 1) as u16;
const GAIN_DB_RANGE: std::ops::RangeInclusive<Sample> = -100.0..=48.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum EditKind {
    Range,
    NthElement,
}

impl EditKind {
    const ALL: &[Self] = &[Self::Range, Self::NthElement];

    fn label(self) -> &'static str {
        match self {
            Self::Range => "Range",
            Self::NthElement => "NthElement",
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
        }
    }
}

impl EditFormState {
    fn to_request(&self) -> EditRequest {
        let gain = self.gain_db.map(db_to_gain);

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
        }
    }
}

pub struct HarmonicEditorUI {
    module_id: ModuleId,
    edit_form: Option<EditFormState>,
}

impl HarmonicEditorUI {
    pub fn new(module_id: ModuleId) -> Self {
        Self {
            module_id,
            edit_form: None,
        }
    }

    fn paint_ui(
        &mut self,
        bridge: &mut UiBridge,
        editor_bridge: &mut HarmonicEditorUiBridge,
        ui: &mut Ui,
    ) {
        let module_id = self.module_id;
        ui.style_mut().spacing.scroll = ScrollStyle::solid();

        Panel::top("harmonics-list")
            .resizable(true)
            .size_range(150.0..=400.0)
            .default_size(200.0)
            .frame(Frame::NONE.inner_margin(Margin {
                left: 0,
                top: 0,
                right: 0,
                bottom: 8,
            }))
            .show(ui, |ui| {
                ScrollArea::horizontal().show(ui, |ui| {
                    ui.horizontal_top(|ui| {
                        let height = ui.available_height();

                        ui.style_mut().spacing.item_spacing = Vec2::splat(2.0);
                        ui.style_mut().interaction.tooltip_delay = 0.1;
                        ui.style_mut().interaction.show_tooltips_only_when_still = false;

                        let mut changed = None;
                        {
                            let harmonics = editor_bridge.harmonics_mut();

                            for idx in 1..SPECTRAL_BUFFER_SIZE {
                                let mut gain = harmonics.amplitude(idx);

                                if ui
                                    .add(
                                        GainSlider::new(&mut gain)
                                            .label(&format!("{}", idx))
                                            .height(height),
                                    )
                                    .changed()
                                {
                                    harmonics.set_amplitude(idx, gain);
                                    changed = Some((idx, gain));
                                }
                            }
                        }

                        if let Some((idx, gain)) = changed {
                            editor_bridge.set_harmonic(idx, gain);
                        }
                    });
                });
            });

        ui.add(ModuleLabel::new(
            module_id,
            ModuleType::HarmonicEditor,
            bridge,
        ));

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
                ui.heading("Set Harmonics");
                ui.add_space(20.0);
                ui.set_width(440.0);

                let mut changed = false;

                Grid::new("harmonic-editor-edit-form")
                    .num_columns(2)
                    .spacing([40.0, 24.0])
                    .striped(true)
                    .show(ui, |ui| {
                        ui.label("");
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
                    });

                if changed {
                    editor_bridge.edit_request(state.to_request());
                }

                ui.add_space(40.0);

                Sides::new().show(
                    ui,
                    |_ui| {},
                    |ui| {
                        if ui.button("Set").clicked() {
                            editor_bridge.apply_draft();
                            saved = true;
                            ui.close();
                        }

                        if ui.button("Discard").clicked() {
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
