use std::collections::HashSet;

use egui_baseview::egui::{ComboBox, Grid, Response, Ui, Widget};

use crate::{
    editor::stereo_slider::StereoSlider,
    synth_engine::{
        ConnectedInputSourceUI, InputType, ModuleInput, ModuleOutput, Sample, StereoSample,
        SynthEngine,
    },
    utils::st_to_octave,
};

pub struct ModulationInput<'a> {
    value: &'a mut StereoSample,
    synth_engine: &'a mut SynthEngine,
    input: ModuleInput,
    modulation_default: Option<Sample>,
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
            modulation_default: None,
        }
    }

    pub fn modulation_default(mut self, default: Sample) -> Self {
        self.modulation_default = Some(default);
        self
    }

    fn setup_value_slider(slider: StereoSlider<'_>, input_type: InputType) -> StereoSlider<'_> {
        match input_type {
            InputType::Level => slider.default_value(1.0).precision(2),
            InputType::Cutoff => slider
                .range(-4.0..=10.0)
                .display_scale(12.0)
                .default_value(0.0)
                .precision(2)
                .units("st"),
            InputType::Q => slider
                .range(0.1..=10.0)
                .default_value(0.7)
                .skew(1.8)
                .precision(2),
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
            InputType::Audio | InputType::Spectrum | InputType::ScalarInput => slider,
        }
    }

    fn setup_modulation_slider(&self, slider: StereoSlider<'a>) -> StereoSlider<'a> {
        let mut updated = match self.input.input_type {
            InputType::Level => slider.default_value(0.0).precision(2).allow_inverse(),
            InputType::Cutoff => slider
                .range(0.0..=8.0)
                .display_scale(12.0)
                .default_value(0.0)
                .precision(2)
                .allow_inverse()
                .units("st"),
            InputType::Q => slider
                .range(0.0..=10.0)
                .default_value(0.0)
                .precision(2)
                .skew(1.8)
                .allow_inverse(),
            InputType::Detune => slider
                .range(0.0..=st_to_octave(1.0))
                .display_scale(1200.0)
                .default_value(st_to_octave(0.2))
                .allow_inverse()
                .units("cents"),
            InputType::PitchShift => slider
                .range(0.0..=st_to_octave(60.0))
                .skew(1.8)
                .display_scale(12.0)
                .default_value(0.0)
                .precision(2)
                .allow_inverse()
                .units("st"),
            InputType::Audio | InputType::Spectrum | InputType::ScalarInput => slider,
        };

        if let Some(default) = self.modulation_default {
            updated = updated.default_value(default);
        }

        updated
    }

    fn add_slider(&mut self, ui: &mut Ui) -> Response {
        ui.add(
            Self::setup_value_slider(StereoSlider::new(self.value), self.input.input_type)
                .width(200.0),
        )
    }

    fn add_modulation(&mut self, src: ModuleOutput) {
        self.synth_engine
            .add_modulation(
                src,
                self.input,
                StereoSample::splat(self.modulation_default.unwrap_or(0.0)),
            )
            .unwrap_or_else(|_| println!("Failed to add modulation"));
    }

    fn add_modulation_select(&mut self, ui: &mut Ui, connected: &[ConnectedInputSourceUI]) {
        let available = self.synth_engine.get_available_input_sources(self.input);
        let connected_ids: HashSet<_> =
            HashSet::from_iter(connected.iter().map(|src| src.output.module_id));
        let filtered: Vec<_> = available
            .iter()
            .filter(|src| !connected_ids.contains(&src.output.module_id))
            .collect();

        if filtered.is_empty() {
            return;
        }

        ComboBox::from_id_salt(format!("mod-select-{:?}", self.input.input_type))
            .selected_text("Add Modulation")
            .show_ui(ui, |ui| {
                for src in &filtered {
                    if ui.selectable_label(false, &src.label).clicked() {
                        self.add_modulation(src.output);
                    }
                }
            });
    }

    fn add_connected_modulations(&mut self, ui: &mut Ui, connected: &[ConnectedInputSourceUI]) {
        Grid::new(format!("mod-grid-{:?}", self.input.input_type))
            .num_columns(3)
            .spacing([8.0, 4.0])
            .striped(false)
            .show(ui, |ui| {
                for src in connected {
                    ui.label(&src.label);

                    let mut modulation = src.modulation;

                    let slider_response = ui.add(
                        self.setup_modulation_slider(StereoSlider::new(&mut modulation))
                            .width(200.0),
                    );

                    if slider_response.changed() {
                        self.synth_engine
                            .update_modulation(&src.output, &self.input, modulation);
                    }

                    if ui.button("X").clicked() {
                        self.synth_engine.remove_link(&src.output, &self.input);
                    }

                    ui.end_row();
                }
            });
    }
}

impl Widget for ModulationInput<'_> {
    fn ui(mut self, ui: &mut Ui) -> Response {
        ui.vertical(|ui| {
            let connected = self.synth_engine.get_connected_input_sources(self.input);
            let result_response = ui
                .horizontal(|ui| {
                    let result_response = self.add_slider(ui);

                    self.add_modulation_select(ui, &connected);
                    result_response
                })
                .inner;

            if !connected.is_empty() {
                self.add_connected_modulations(ui, &connected);
            }

            result_response
        })
        .inner
    }
}
