use egui_baseview::egui::{
    CentralPanel, Frame, Margin, Response, ScrollArea, TopBottomPanel, Ui, Vec2, Widget,
    style::ScrollStyle,
};

use crate::{
    editor::gain_slider::GainSlider,
    synth_engine::{HarmonicEditor, ModuleId, SynthEngine},
};

pub struct HarmonicEditorUI<'a> {
    module_id: ModuleId,
    synth_engine: &'a mut SynthEngine,
}

impl<'a> HarmonicEditorUI<'a> {
    pub fn new(module_id: ModuleId, synth_engine: &'a mut SynthEngine) -> Self {
        Self {
            module_id,
            synth_engine,
        }
    }

    fn editor(&mut self) -> &mut HarmonicEditor {
        HarmonicEditor::downcast_mut_unwrap(self.synth_engine.get_module_mut(self.module_id))
    }
}

impl Widget for HarmonicEditorUI<'_> {
    fn ui(mut self, ui: &mut Ui) -> Response {
        let mut need_update = false;

        ui.style_mut().spacing.scroll = ScrollStyle::solid();

        let response = TopBottomPanel::top("harmonics-list")
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
                        let harmonics = self.editor().harmonics_ref_mut();
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
            })
            .response;

        CentralPanel::default().show_inside(ui, |ui| {
            ui.heading("Harmonics editor");
        });

        if need_update {
            self.editor().apply_harmonics();
        }

        response
    }
}
