use egui_baseview::egui::{
    CentralPanel, Frame, Margin, ScrollArea, TopBottomPanel, Ui, Vec2, style::ScrollStyle,
};

use crate::{
    editor::{
        ModuleUI, gain_slider::GainSlider, module_label::ModuleLabel, utils::confirm_module_removal,
    },
    synth_engine::{HarmonicEditor, ModuleId, SynthEngine},
};

pub struct HarmonicEditorUI {
    module_id: ModuleId,
    remove_confirmation: bool,
    label_state: Option<String>,
}

impl HarmonicEditorUI {
    pub fn new(module_id: ModuleId) -> Self {
        Self {
            module_id,
            remove_confirmation: false,
            label_state: None,
        }
    }

    fn editor<'a>(&mut self, synth: &'a mut SynthEngine) -> &'a mut HarmonicEditor {
        HarmonicEditor::downcast_mut_unwrap(synth.get_module_mut(self.module_id))
    }
}

impl ModuleUI for HarmonicEditorUI {
    fn module_id(&self) -> ModuleId {
        self.module_id
    }

    fn ui(&mut self, synth: &mut SynthEngine, ui: &mut Ui) {
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
                        let harmonics = self.editor(synth).harmonics_ref_mut();
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
            let module = synth.get_module_mut(self.module_id).unwrap();

            ui.add(ModuleLabel::new(
                &module.label(),
                &mut self.label_state,
                module,
            ));
        });

        if need_update {
            self.editor(synth).apply_harmonics();
        }

        ui.add_space(40.0);

        if confirm_module_removal(ui, &mut self.remove_confirmation) {
            synth.remove_module(self.module_id);
        }
    }
}
