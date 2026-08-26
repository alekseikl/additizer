use crate::{
    editor::{
        ModuleUi, gain_slider::GainSlider, module_label::ModuleLabel, slider::Slider, units::Units,
    },
    synth_engine::{
        ModuleId, ModuleType, SPECTRAL_BUFFER_SIZE, StereoSample,
        harmonic_editor::{HarmonicEditorUiBridge, SetAction, SetParams},
        ui_bridge::{ModuleBridge, UiBridge},
    },
    utils::{NthElement, db_to_gain},
};
use egui::{
    Checkbox, ComboBox, DragValue, Frame, Grid, Id, Margin, Modal, Panel, ScrollArea, Sides, Ui,
    Vec2, style::ScrollStyle,
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

pub struct HarmonicEditorUI {
    module_id: ModuleId,
    select_and_set_state: Option<Box<SelectAndSetState>>,
}

impl HarmonicEditorUI {
    pub fn new(module_id: ModuleId) -> Self {
        Self {
            module_id,
            select_and_set_state: None,
        }
    }

    fn apply_select_and_set(bridge: &mut HarmonicEditorUiBridge, state: &SelectAndSetState) {
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

        bridge.set_selected(params);
    }

    fn show_select_and_set_modal(
        bridge: &mut HarmonicEditorUiBridge,
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
                        Slider::stereo(&mut state.volume, -100.0..=40.0, None)
                            .default(0.0)
                            .skew(1.6)
                            .units(Units::Db),
                    );
                    ui.end_row();
                });

            ui.add_space(40.0);

            Sides::new().show(
                ui,
                |_ui| {},
                |ui| {
                    if ui.button("Ok").clicked() {
                        Self::apply_select_and_set(bridge, state);
                        ui.close();
                    }

                    if ui.button("Apply").clicked() {
                        Self::apply_select_and_set(bridge, state);
                    }

                    if ui.button("Cancel").clicked() {
                        ui.close();
                    }
                },
            );
        });

        !modal.should_close()
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
                        let mut harmonics = editor_bridge.harmonics();
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
                                editor_bridge.set_harmonic(idx, *harmonic);
                            }
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
            if ui.button("All to Zero").clicked() {
                editor_bridge.set_selected(SetParams {
                    from: 1,
                    to: NUM_EDITABLE_HARMONICS,
                    n_th: None,
                    action: SetAction::Set,
                    gain: StereoSample::splat(0.0),
                });
            }

            if ui.button("All to One").clicked() {
                editor_bridge.set_selected(SetParams {
                    from: 1,
                    to: NUM_EDITABLE_HARMONICS,
                    n_th: None,
                    action: SetAction::Set,
                    gain: StereoSample::splat(1.0),
                });
            }

            if ui.button("Keep Even").clicked() {
                editor_bridge.set_selected(SetParams {
                    from: 1,
                    to: NUM_EDITABLE_HARMONICS,
                    n_th: Some(NthElement::new(2, 0, true)),
                    action: SetAction::Set,
                    gain: StereoSample::splat(0.0),
                });
            }

            if ui.button("Keep Odd").clicked() {
                editor_bridge.set_selected(SetParams {
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
        });

        if let Some(mut state) = self.select_and_set_state.take()
            && Self::show_select_and_set_modal(editor_bridge, ui, &mut state)
        {
            self.select_and_set_state.replace(state);
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
