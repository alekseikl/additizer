use egui_baseview::egui::{
    CentralPanel, Checkbox, ComboBox, DragValue, Frame, Grid, Id, Margin, Modal, ScrollArea, Sides,
    TopBottomPanel, Ui, Vec2, style::ScrollStyle,
};
use nih_plug::util::db_to_gain;

use crate::{
    editor::{
        ModuleUI, gain_slider::GainSlider, module_label::ModuleLabel, stereo_slider::StereoSlider,
        utils::confirm_module_removal,
    },
    synth_engine::{
        HarmonicEditor, ModuleId, SPECTRAL_BUFFER_SIZE, StereoSample, SynthEngine,
        harmonic_editor::{FilterParams, FilterType, SetAction, SetParams},
    },
    utils::NthElement,
};

const NUM_EDITABLE_HARMONICS: usize = SPECTRAL_BUFFER_SIZE - 1;

impl SetAction {
    fn label(&self) -> &'static str {
        match self {
            Self::Set => "Set",
            Self::Multiple => "Multiple",
        }
    }
}

impl FilterType {
    fn label(&self) -> &'static str {
        match self {
            Self::LowPass => "Lowpass",
            Self::HighPass => "Highpass",
            Self::BandPass => "Bandpass",
            Self::BandStop => "Bandstop",
            Self::Peaking => "Peaking",
        }
    }
}

struct SelectAndSetState {
    from: usize,
    to: usize,
    n_th_element: bool,
    n_th_mul: isize,
    n_th_add: isize,
    n_th_inverted: bool,
    action: SetAction,
    volume: StereoSample,
}

impl Default for SelectAndSetState {
    fn default() -> Self {
        Self {
            from: 1,
            to: NUM_EDITABLE_HARMONICS,
            n_th_element: false,
            n_th_mul: 2,
            n_th_add: 1,
            n_th_inverted: false,
            action: SetAction::Set,
            volume: StereoSample::splat(0.0),
        }
    }
}

struct ApplyFilterState {
    filter_type: FilterType,
    order: StereoSample,
    cutoff: StereoSample,
    q: StereoSample,
    gain: StereoSample,
}

impl Default for ApplyFilterState {
    fn default() -> Self {
        Self {
            filter_type: FilterType::LowPass,
            order: 4.0.into(),
            cutoff: 0.0.into(),
            q: 0.707.into(),
            gain: 0.0.into(),
        }
    }
}

pub struct HarmonicEditorUI {
    module_id: ModuleId,
    remove_confirmation: bool,
    label_state: Option<String>,
    select_and_set_state: Option<Box<SelectAndSetState>>,
    apply_filter_state: Option<Box<ApplyFilterState>>,
}

impl HarmonicEditorUI {
    pub fn new(module_id: ModuleId) -> Self {
        Self {
            module_id,
            remove_confirmation: false,
            label_state: None,
            select_and_set_state: None,
            apply_filter_state: None,
        }
    }

    fn editor<'a>(&self, synth: &'a mut SynthEngine) -> &'a mut HarmonicEditor {
        HarmonicEditor::downcast_mut_unwrap(synth.get_module_mut(self.module_id))
    }

    fn apply_select_and_set(&self, synth: &mut SynthEngine, state: &SelectAndSetState) {
        let mut params = SetParams {
            from: state.from,
            to: state.to,
            n_th: None,
            action: state.action,
            gain: state
                .volume
                .iter()
                .map(|volume| db_to_gain(*volume))
                .collect(),
        };

        if state.n_th_element {
            params.n_th = Some(NthElement::new(
                state.n_th_mul,
                state.n_th_add,
                state.n_th_inverted,
            ))
        }

        self.editor(synth).set_selected(&params);
    }

    fn show_select_and_set_modal(
        &mut self,
        synth: &mut SynthEngine,
        ui: &mut Ui,
        state: &mut SelectAndSetState,
    ) -> bool {
        let modal = Modal::new(Id::new("set-and-select-modal")).show(ui.ctx(), |ui| {
            ui.set_width(440.0);

            Grid::new("set-and-select-modal")
                .num_columns(2)
                .spacing([40.0, 24.0])
                .striped(true)
                .show(ui, |ui| {
                    ui.label("Harmonics");
                    ui.horizontal(|ui| {
                        ui.add(DragValue::new(&mut state.from).range(1..=NUM_EDITABLE_HARMONICS));
                        ui.label(" — ");
                        ui.add(DragValue::new(&mut state.to).range(1..=NUM_EDITABLE_HARMONICS));
                    });
                    ui.end_row();

                    ui.label("N-th Element");
                    ui.horizontal(|ui| {
                        ui.add(Checkbox::without_text(&mut state.n_th_element));

                        if state.n_th_element {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 0.0;
                                ui.add(DragValue::new(&mut state.n_th_mul).range(2..=50));
                                ui.label("n + ");
                                ui.add(
                                    DragValue::new(&mut state.n_th_add)
                                        .range(0..=(state.n_th_mul - 1)),
                                );
                            });

                            ui.add(Checkbox::new(&mut state.n_th_inverted, "Inverted"));
                        }
                    });
                    ui.end_row();

                    ui.label("Action");
                    ComboBox::from_id_salt("select-and-set-action")
                        .selected_text(state.action.label())
                        .show_ui(ui, |ui| {
                            const ACTION_OPTIONS: &[SetAction] =
                                &[SetAction::Set, SetAction::Multiple];

                            for action in ACTION_OPTIONS {
                                ui.selectable_value(&mut state.action, *action, action.label());
                            }
                        });
                    ui.end_row();

                    ui.label("Volume");
                    ui.add(
                        StereoSlider::new(&mut state.volume)
                            .range(-100.0..=40.0)
                            .default_value(0.0)
                            .skew(1.6)
                            .units("dB"),
                    );
                    ui.end_row();
                });

            ui.add_space(40.0);

            Sides::new().show(
                ui,
                |_ui| {},
                |ui| {
                    if ui.button("Ok").clicked() {
                        self.apply_select_and_set(synth, state);
                        ui.close();
                    }

                    if ui.button("Apply").clicked() {
                        self.apply_select_and_set(synth, state);
                    }

                    if ui.button("Cancel").clicked() {
                        ui.close();
                    }
                },
            );
        });

        !modal.should_close()
    }

    fn apply_filter(&self, synth: &mut SynthEngine, state: &ApplyFilterState) {
        self.editor(synth).apply_filter(&FilterParams {
            filter_type: state.filter_type,
            filter_order: state.order,
            cutoff: state.cutoff.iter().map(|octave| octave.exp2()).collect(),
            q: state.q,
            level: state
                .gain
                .iter()
                .map(|volume| db_to_gain(*volume))
                .collect(),
        });
    }

    fn show_apply_filter_modal(
        &mut self,
        synth: &mut SynthEngine,
        ui: &mut Ui,
        state: &mut ApplyFilterState,
    ) -> bool {
        let modal = Modal::new(Id::new("apply-filter-modal")).show(ui.ctx(), |ui| {
            ui.set_width(440.0);

            Grid::new("set-and-select-modal")
                .num_columns(2)
                .spacing([40.0, 24.0])
                .striped(true)
                .show(ui, |ui| {
                    ui.label("Type");
                    ComboBox::from_id_salt("apply-filter-type")
                        .selected_text(state.filter_type.label())
                        .show_ui(ui, |ui| {
                            const TYPE_OPTIONS: &[FilterType] = &[
                                FilterType::LowPass,
                                FilterType::HighPass,
                                FilterType::BandPass,
                                FilterType::BandStop,
                                FilterType::Peaking,
                            ];

                            for filter_type in TYPE_OPTIONS {
                                ui.selectable_value(
                                    &mut state.filter_type,
                                    *filter_type,
                                    filter_type.label(),
                                );
                            }
                        });
                    ui.end_row();

                    ui.label("Order");
                    ui.add(
                        StereoSlider::new(&mut state.order)
                            .range(1.0..=8.0)
                            .default_value(4.0),
                    );
                    ui.end_row();

                    ui.label("Cutoff");
                    ui.add(
                        StereoSlider::new(&mut state.cutoff)
                            .range(-4.0..=10.0)
                            .display_scale(12.0)
                            .default_value(0.0)
                            .precision(2)
                            .units(" st"),
                    );
                    ui.end_row();

                    ui.label("Q");
                    ui.add(
                        StereoSlider::new(&mut state.q)
                            .range(0.1..=10.0)
                            .default_value(0.707)
                            .skew(1.8)
                            .precision(2),
                    );
                    ui.end_row();

                    ui.label("Gain");
                    ui.add(
                        StereoSlider::new(&mut state.gain)
                            .range(0.0..=24.0)
                            .default_value(0.0)
                            .skew(1.6)
                            .allow_inverse()
                            .units(" dB"),
                    );
                    ui.end_row();
                });

            ui.add_space(40.0);

            Sides::new().show(
                ui,
                |_ui| {},
                |ui| {
                    if ui.button("Ok").clicked() {
                        self.apply_filter(synth, state);
                        ui.close();
                    }

                    if ui.button("Apply").clicked() {
                        self.apply_filter(synth, state);
                    }

                    if ui.button("Cancel").clicked() {
                        ui.close();
                    }
                },
            );
        });

        !modal.should_close()
    }
}

impl ModuleUI for HarmonicEditorUI {
    fn module_id(&self) -> ModuleId {
        self.module_id
    }

    fn ui(&mut self, synth: &mut SynthEngine, ui: &mut Ui) {
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
                        let mut harmonics = self.editor(synth).get_harmonics();
                        let height = ui.available_height();

                        ui.style_mut().spacing.item_spacing = Vec2::splat(2.0);
                        ui.style_mut().interaction.tooltip_delay = 0.1;
                        ui.style_mut().interaction.show_tooltips_only_when_still = false;

                        for (idx, harmonic) in harmonics.iter_mut().enumerate().skip(1) {
                            if ui
                                .add(
                                    GainSlider::new(harmonic)
                                        .label(&format!("{}", idx))
                                        .height(height),
                                )
                                .changed()
                            {
                                self.editor(synth).set_harmonic(idx, *harmonic);
                            }
                        }
                    });
                });
            });

        CentralPanel::default().show_inside(ui, |ui| {
            let module = synth.get_module_mut(self.module_id).unwrap();

            ui.add(ModuleLabel::new(
                &module.label(),
                &mut self.label_state,
                module,
            ));
        });

        ui.add_space(60.0);

        ui.horizontal(|ui| {
            if ui.button("All to Zero").clicked() {
                self.editor(synth).set_selected(&SetParams {
                    from: 1,
                    to: NUM_EDITABLE_HARMONICS,
                    n_th: None,
                    action: SetAction::Set,
                    gain: StereoSample::splat(0.0),
                });
            }

            if ui.button("All to One").clicked() {
                self.editor(synth).set_selected(&SetParams {
                    from: 1,
                    to: NUM_EDITABLE_HARMONICS,
                    n_th: None,
                    action: SetAction::Set,
                    gain: StereoSample::splat(1.0),
                });
            }

            if ui.button("Keep Even").clicked() {
                self.editor(synth).set_selected(&SetParams {
                    from: 1,
                    to: NUM_EDITABLE_HARMONICS,
                    n_th: Some(NthElement::new(2, 0, true)),
                    action: SetAction::Set,
                    gain: StereoSample::splat(0.0),
                });
            }

            if ui.button("Keep Odd").clicked() {
                self.editor(synth).set_selected(&SetParams {
                    from: 1,
                    to: NUM_EDITABLE_HARMONICS,
                    n_th: Some(NthElement::new(2, 1, true)),
                    action: SetAction::Set,
                    gain: StereoSample::splat(0.0),
                });
            }
        });

        ui.horizontal(|ui| {
            if ui.button("Select and Set").clicked() {
                self.select_and_set_state = Some(Box::new(SelectAndSetState::default()));
            }

            if ui.button("Apply Filter").clicked() {
                self.apply_filter_state = Some(Box::new(ApplyFilterState::default()));
            }
        });

        if let Some(mut state) = self.select_and_set_state.take()
            && self.show_select_and_set_modal(synth, ui, &mut state)
        {
            self.select_and_set_state.replace(state);
        }

        if let Some(mut state) = self.apply_filter_state.take()
            && self.show_apply_filter_modal(synth, ui, &mut state)
        {
            self.apply_filter_state.replace(state);
        }

        ui.add_space(40.0);

        if confirm_module_removal(ui, &mut self.remove_confirmation) {
            synth.remove_module(self.module_id);
        }
    }
}
