use egui::{Checkbox, ComboBox, Grid, Ui};

use crate::{
    editor::{
        ModuleUi, module_label::ModuleLabel, stereo_slider::StereoSlider,
        utils::confirm_module_removal,
    },
    synth_engine::{ModuleId, StereoSample, external_param::{self, NUM_FLOAT_PARAMS}, ui_bridge::UiBridge},
};

pub struct ExternalParamUI {
    remove_confirmation: bool,
    label_state: Option<String>,
    param_bridge: external_param::UiBridge,
}

impl ExternalParamUI {
    pub fn new(module_id: ModuleId, synth_bridge: &mut UiBridge) -> Option<Self> {
        let param_bridge =
            external_param::UiBridge::create(module_id, synth_bridge.synth().clone())?;

        Some(Self {
            remove_confirmation: false,
            label_state: None,
            param_bridge,
        })
    }
}

impl ModuleUi for ExternalParamUI {
    fn module_id(&self) -> Option<ModuleId> {
        Some(self.param_bridge.module_id())
    }

    fn ui(&mut self, bridge: &mut UiBridge, ui: &mut Ui) {
        let module_id = self.param_bridge.module_id();
        let mut config = self.param_bridge.config().clone();

        ui.add(ModuleLabel::new(
            &mut self.label_state,
            bridge,
            module_id,
        ));

        ui.add_space(20.0);

        Grid::new("ext-param-grid")
            .num_columns(2)
            .spacing([40.0, 24.0])
            .striped(true)
            .show(ui, |ui| {
                ui.label("Input");
                ComboBox::from_id_salt("ext-param-select")
                    .selected_text(format!("Param #{}", config.selected_param_index + 1))
                    .show_ui(ui, |ui| {
                        for i in 0..NUM_FLOAT_PARAMS {
                            if ui
                                .selectable_label(
                                    i == config.selected_param_index,
                                    format!("Param #{}", i + 1),
                                )
                                .clicked()
                            {
                                self.param_bridge.select_param(i);
                            }
                        }
                    });
                ui.end_row();

                let mut smooth = StereoSample::splat(config.smooth);

                ui.label("Smooth");
                if ui
                    .add(
                        StereoSlider::new(&mut smooth)
                            .range(0.0..=0.05)
                            .display_scale(1000.0)
                            .default_value(0.0)
                            .skew(1.2)
                            .precision(1)
                            .units(" ms"),
                    )
                    .changed()
                {
                    self.param_bridge.set_smooth(smooth.left());
                }
                ui.end_row();

                ui.label("Sample and Hold");
                if ui
                    .add(Checkbox::without_text(&mut config.sample_and_hold))
                    .changed()
                {
                    self.param_bridge
                        .set_sample_and_hold(config.sample_and_hold);
                }
                ui.end_row();
            });

        ui.add_space(40.0);

        if confirm_module_removal(ui, &mut self.remove_confirmation) {
            bridge.remove_module(module_id);
        }
    }
}
