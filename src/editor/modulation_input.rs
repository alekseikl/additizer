use egui_baseview::egui::{Response, Ui, Widget};

use crate::{
    editor::stereo_slider::StereoSlider,
    synth_engine::{InputType, ModuleInput, StereoSample, SynthEngine},
    utils::st_to_octave,
};

pub struct ModulationInput<'a> {
    value: &'a mut StereoSample,
    synth_engine: &'a mut SynthEngine,
    input: ModuleInput,
}

impl<'a> ModulationInput<'a> {
    pub fn new(
        value: &'a mut StereoSample,
        synth_engine: &'a mut SynthEngine,
        input: ModuleInput,
    ) -> Self {
        Self {
            value,
            synth_engine,
            input,
        }
    }

    fn setup_value_slider(mut slider: StereoSlider<'a>, input_type: InputType) -> StereoSlider<'a> {
        match input_type {
            InputType::Level => slider.default_value(1.0).precision(2),
            InputType::CutoffScalar => slider
                .range(-4.0..=10.0)
                .display_scale(12.0)
                .default_value(0.0)
                .precision(2)
                .units("st"),
            InputType::Detune => slider
                .range(0.0..=st_to_octave(1.0))
                .display_scale(1200.0)
                .default_value(st_to_octave(0.2))
                .units("cents"),
            InputType::PitchShift => slider
                .range(0.0..=st_to_octave(60.0))
                .skew(1.6)
                .display_scale(12.0)
                .default_value(0.0)
                .precision(2)
                .allow_inverse()
                .units("st"),
            InputType::Input | InputType::Spectrum => slider,
        }
    }
}

impl Widget for ModulationInput<'_> {
    fn ui(mut self, ui: &mut Ui) -> Response {
        let slider_response = ui.add(
            Self::setup_value_slider(StereoSlider::new(self.value), self.input.input_type)
                .width(200.0),
        );

        slider_response
    }
}
