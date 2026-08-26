use crate::{
    editor::{ModuleUi, gain_slider::GainSlider, module_label::ModuleLabel},
    synth_engine::{
        ModuleId, ModuleType, SPECTRAL_BUFFER_SIZE, harmonic_editor::HarmonicEditorUiBridge,
        ui_bridge::{ModuleBridge, UiBridge},
    },
};
use egui::{Frame, Margin, Panel, ScrollArea, Ui, Vec2, style::ScrollStyle};

pub struct HarmonicEditorUI {
    module_id: ModuleId,
}

impl HarmonicEditorUI {
    pub fn new(module_id: ModuleId) -> Self {
        Self { module_id }
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
        });
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
