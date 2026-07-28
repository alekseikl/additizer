use egui::{Response, Ui, Widget};

use crate::synth_engine::{Input, InputId, ModuleId, StereoSample, ui_bridge::UiBridge};

pub struct StereoInput<'a> {
    input: InputId,
    value: &'a mut StereoSample,
    bridge: &'a mut UiBridge,
}

impl<'a> StereoInput<'a> {
    pub fn new(
        input: Input,
        module_id: ModuleId,
        value: &'a mut StereoSample,
        bridge: &'a mut UiBridge,
    ) -> Self {
        Self {
            input: InputId::new(input, module_id),
            value,
            bridge,
        }
    }
}

impl Widget for StereoInput<'_> {
    fn ui(mut self, ui: &mut Ui) -> Response {
        ui.horizontal(|ui| {
            let modulated = self.bridge.get_input_modulated_value(self.input);
            let mut slider = self.input.input_type.param_slider(self.value);

            if let Some(modulated) = modulated {
                if modulated.is_stereo {
                    slider = slider.modulated(modulated.value);
                } else {
                    slider = slider.modulated(StereoSample::splat(modulated.value.left()));
                }
            }

            ui.add(slider)
        })
        .inner
    }
}
