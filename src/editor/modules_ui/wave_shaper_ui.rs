use egui::{ComboBox, Grid, Ui};

use crate::{
    editor::{module_label::ModuleLabel, stereo_input::StereoInput, ModuleUi},
    synth_engine::{
        ui_bridge::{ModuleBridge, UiBridge},
        wave_shaper::WaveShaperUiBridge,
        Input, ModuleId, ModuleType, ShaperType,
    },
};

impl ShaperType {
    fn label(&self) -> &'static str {
        match self {
            Self::HardClip => "Hard Clip",
            Self::Sigmoid => "Sigmoid",
        }
    }
}

pub struct WaveShaperUi {
    module_id: ModuleId,
}

impl WaveShaperUi {
    pub fn new(module_id: ModuleId) -> Self {
        Self { module_id }
    }

    fn paint_ui(
        &mut self,
        bridge: &mut UiBridge,
        shaper_bridge: &mut WaveShaperUiBridge,
        ui: &mut Ui,
    ) {
        let module_id = self.module_id;
        let mut config = shaper_bridge.config().clone();

        ui.add(ModuleLabel::new(module_id, ModuleType::WaveShaper, bridge));

        ui.add_space(16.0);

        Grid::new("waveshaper_grid")
            .num_columns(2)
            .spacing([40.0, 24.0])
            .striped(true)
            .show(ui, |ui| {
                ui.label("Type");
                ComboBox::from_id_salt("waveshaper-type")
                    .selected_text(config.shaper_type.label())
                    .show_ui(ui, |ui| {
                        const TYPE_OPTIONS: &[ShaperType] =
                            &[ShaperType::HardClip, ShaperType::Sigmoid];

                        for shaper_type in TYPE_OPTIONS {
                            if ui
                                .selectable_value(
                                    &mut config.shaper_type,
                                    *shaper_type,
                                    shaper_type.label(),
                                )
                                .clicked()
                            {
                                shaper_bridge.set_shaper_type(*shaper_type);
                            }
                        }
                    });
                ui.end_row();

                ui.label("Distortion");
                if ui
                    .add(StereoInput::new(
                        Input::Distortion,
                        module_id,
                        &mut config.distortion,
                        bridge,
                    ))
                    .changed()
                {
                    shaper_bridge.set_param(Input::Distortion, config.distortion);
                }
                ui.end_row();

                ui.label("Clipping level");
                if ui
                    .add(StereoInput::new(
                        Input::ClippingLevel,
                        module_id,
                        &mut config.clipping_level,
                        bridge,
                    ))
                    .changed()
                {
                    shaper_bridge.set_param(Input::ClippingLevel, config.clipping_level);
                }
                ui.end_row();
            });
    }
}

impl ModuleUi for WaveShaperUi {
    fn module_id(&self) -> Option<ModuleId> {
        Some(self.module_id)
    }

    fn ui(&mut self, bridge: &mut UiBridge, ui: &mut Ui) {
        bridge.with_module_bridge(self.module_id, |bridge, module_bridge| {
            if let ModuleBridge::WaveShaper(shaper_bridge) = module_bridge {
                self.paint_ui(bridge, shaper_bridge, ui);
            }
        });
    }
}
