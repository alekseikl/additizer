use egui::{Response, Sense, Ui, Widget, emath, lerp, vec2};

use crate::{
    editor::grid::input_mixer_popup::InputMixerPopup,
    synth_engine::{
        Input, InputId, ModuleId, ModuleType, Sample, StereoSample,
        ui_bridge::{ModulatedValue, UiBridge},
    },
};

const IO_DOT_SIZE: f32 = 8.0;
const IO_DOT_SIZE_HOVER: f32 = 10.0;

pub struct StereoInput<'a> {
    input: InputId,
    value: &'a mut StereoSample,
    bridge: &'a mut UiBridge,
    default: Option<Sample>,
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
            default: None,
        }
    }

    pub fn default(mut self, default: Sample) -> Self {
        self.default = Some(default);
        self
    }
}

impl StereoInput<'_> {
    fn module_type(&self) -> Option<ModuleType> {
        self.bridge
            .get_modules()
            .into_iter()
            .find(|m| m.id == self.input.module_id)
            .map(|m| m.module_type)
    }

    fn add_circle(&mut self, ui: &mut Ui, modulated: Option<&ModulatedValue>) {
        if !self.bridge.has_connected_input_sources(self.input) {
            ui.allocate_exact_size(vec2(IO_DOT_SIZE_HOVER, IO_DOT_SIZE_HOVER), Sense::hover());
            return;
        }

        let (rect, response) =
            ui.allocate_exact_size(vec2(IO_DOT_SIZE_HOVER, IO_DOT_SIZE_HOVER), Sense::click());
        let mixer_id = response.id;

        if response.clicked() {
            ui.data_mut(|d| d.insert_temp(mixer_id, true));
        }

        let is_open = ui
            .ctx()
            .data_mut(|d| d.remove_temp(mixer_id).unwrap_or_default());

        let still_open = if is_open && let Some(module_type) = self.module_type() {
            let popup = InputMixerPopup {
                module_id: self.input.module_id,
                module_type,
                input: self.input.input_type,
            };

            !popup.show(&response, ui, self.bridge)
        } else {
            false
        };

        if still_open {
            ui.data_mut(|d| d.insert_temp(mixer_id, true));
        }

        let t = ui.ctx().animate_bool_with_time_and_easing(
            response.id,
            response.hovered(),
            0.15,
            emath::easing::cubic_out,
        );
        let dot_size = lerp(IO_DOT_SIZE..=IO_DOT_SIZE_HOVER, t);
        let blend = modulated
            .map(|m| m.normalized.left().max(m.normalized.right()))
            .unwrap_or(0.0);
        let dot_color = self
            .input
            .input_type
            .color()
            .lerp_to_gamma(egui::Color32::WHITE, blend);

        ui.painter()
            .circle_filled(rect.center(), dot_size * 0.5, dot_color);
    }
}

impl Widget for StereoInput<'_> {
    fn ui(mut self, ui: &mut Ui) -> Response {
        ui.horizontal_centered(|ui| {
            let modulated = self.bridge.get_input_modulated_value(self.input);
            let mut slider = self.input.input_type.param_slider(self.value);

            if let Some(default) = self.default {
                slider = slider.default(default);
            }

            if let Some(modulated) = modulated.as_ref() {
                if modulated.is_stereo {
                    slider = slider.modulated(modulated.value);
                } else {
                    slider = slider.modulated(StereoSample::splat(modulated.value.left()));
                }
            }

            let response = ui.add(slider);

            self.add_circle(ui, modulated.as_ref());

            response
        })
        .inner
    }
}
