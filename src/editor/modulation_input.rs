use std::collections::HashSet;

use egui_baseview::egui::{ComboBox, Frame, Grid, Margin, Response, Ui, Widget};

use crate::{
    editor::stereo_slider::StereoSlider,
    synth_engine::{
        ConnectedInputSourceUI, Input, ModuleId, ModuleInput, Sample, StereoSample, SynthEngine,
    },
    utils::st_to_octave,
};

pub struct ModulationInput<'a> {
    value: &'a mut StereoSample,
    synth_engine: &'a mut SynthEngine,
    input: ModuleInput,
    default: Option<Sample>,
    modulation_default: Option<Sample>,
}

impl<'a> ModulationInput<'a> {
    pub fn new(
        value: &'a mut StereoSample,
        synth_engine: &'a mut SynthEngine,
        input: Input,
        module_id: ModuleId,
    ) -> Self {
        Self {
            value,
            synth_engine,
            input: ModuleInput::new(input, module_id),
            default: None,
            modulation_default: None,
        }
    }

    pub fn default(mut self, default: Sample) -> Self {
        self.default = Some(default);
        self
    }

    pub fn modulation_default(mut self, default: Sample) -> Self {
        self.modulation_default = Some(default);
        self
    }

    fn setup_value_slider(
        slider: StereoSlider<'_>,
        input_type: Input,
        default: Option<Sample>,
    ) -> StereoSlider<'_> {
        let mut updated = match input_type {
            Input::Level => slider.default_value(1.0).precision(2),
            Input::GainDb => slider
                .range(0.0..=24.0)
                .default_value(0.0)
                .skew(1.6)
                .allow_inverse()
                .units(" dB"),
            Input::Blend => slider.range(0.0..=1.0).default_value(1.0).precision(2),
            Input::Cutoff => slider
                .range(-2.0..=10.0)
                .display_scale(12.0)
                .default_value(0.0)
                .precision(2)
                .units(" st"),
            Input::Q => slider
                .range(0.1..=10.0)
                .default_value(0.707)
                .skew(1.8)
                .precision(2),
            Input::Detune => slider
                .range(0.0..=st_to_octave(1.0))
                .display_scale(1200.0)
                .default_value(st_to_octave(0.2))
                .units(" cents"),
            Input::PitchShift => slider
                .range(0.0..=st_to_octave(60.0))
                .skew(1.6)
                .display_scale(12.0)
                .default_value(0.0)
                .precision(2)
                .allow_inverse()
                .units(" st"),
            Input::PhaseShift => slider.default_value(0.0).precision(2).allow_inverse(),
            Input::LowFrequency => slider
                .range(0.0..=100.0)
                .default_value(1.0)
                .precision(2)
                .allow_inverse()
                .units(" Hz")
                .skew(1.8),
            Input::Skew => slider.default_value(0.5).precision(2),
            Input::Sustain => slider
                .default_value(0.5)
                .display_scale(100.0)
                .precision(2)
                .units("%"),
            Input::Attack | Input::Hold | Input::Decay | Input::Release => slider
                .range(0.0..=8.0)
                .display_scale(1000.0)
                .default_value(0.0)
                .skew(2.0)
                .precision(1)
                .units(" ms"),
            Input::Audio | Input::Spectrum | Input::SpectrumTo => slider,
        };

        if let Some(default) = default {
            updated = updated.default_value(default);
        }

        updated
    }

    fn setup_modulation_slider(&self, slider: StereoSlider<'a>) -> StereoSlider<'a> {
        let mut updated = match self.input.input_type {
            Input::Level => slider.default_value(0.0).precision(2).allow_inverse(),
            Input::GainDb => slider
                .range(0.0..=24.0)
                .default_value(0.0)
                .skew(1.6)
                .allow_inverse()
                .units(" dB"),
            Input::Blend => slider
                .range(0.0..=1.0)
                .default_value(1.0)
                .precision(2)
                .allow_inverse(),
            Input::Cutoff => slider
                .range(0.0..=8.0)
                .display_scale(12.0)
                .default_value(0.0)
                .precision(2)
                .allow_inverse()
                .units(" st"),
            Input::Q => slider
                .range(0.0..=10.0)
                .default_value(0.0)
                .precision(2)
                .skew(1.8)
                .allow_inverse(),
            Input::Detune => slider
                .range(0.0..=st_to_octave(1.0))
                .display_scale(1200.0)
                .default_value(st_to_octave(0.2))
                .allow_inverse()
                .units(" cents"),
            Input::PitchShift => slider
                .range(0.0..=st_to_octave(60.0))
                .skew(1.8)
                .display_scale(12.0)
                .default_value(0.0)
                .precision(2)
                .allow_inverse()
                .units(" st"),
            Input::PhaseShift => slider.default_value(0.0).precision(2).allow_inverse(),
            Input::LowFrequency => slider
                .range(0.0..=100.0)
                .default_value(1.0)
                .precision(2)
                .allow_inverse()
                .units(" Hz")
                .skew(1.8),
            Input::Skew => slider.default_value(0.0).precision(2).allow_inverse(),
            Input::Sustain => slider
                .default_value(0.5)
                .display_scale(100.0)
                .precision(2)
                .units("%"),
            Input::Attack | Input::Hold | Input::Decay | Input::Release => slider
                .range(0.0..=8.0)
                .display_scale(1000.0)
                .default_value(0.0)
                .skew(2.0)
                .precision(1)
                .allow_inverse()
                .units(" ms"),
            Input::Audio | Input::Spectrum | Input::SpectrumTo => slider,
        };

        if let Some(default) = self.modulation_default {
            updated = updated.default_value(default);
        }

        updated
    }

    fn add_slider(&mut self, ui: &mut Ui) -> Response {
        ui.add(
            Self::setup_value_slider(
                StereoSlider::new(self.value),
                self.input.input_type,
                self.default,
            )
            .width(200.0),
        )
    }

    fn add_modulation(&mut self, src: ModuleId) {
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
        let connected_ids: HashSet<_> = HashSet::from_iter(connected.iter().map(|src| src.output));
        let filtered: Vec<_> = available
            .iter()
            .filter(|src| !connected_ids.contains(&src.output))
            .collect();

        if filtered.is_empty() {
            return;
        }

        ComboBox::from_id_salt(format!("mod-select-{:?}", self.input.input_type))
            .selected_text("➕")
            .width(0.0)
            .show_ui(ui, |ui| {
                for src in &filtered {
                    if ui.selectable_label(false, &src.label).clicked() {
                        self.add_modulation(src.output);
                    }
                }
            })
            .response
            .on_hover_text("Add Modulation Source");
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

                    if ui.button("❌").on_hover_text("Remove Modulation").clicked() {
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
                Frame::default()
                    .outer_margin(Margin {
                        left: 8,
                        top: 8,
                        right: 0,
                        bottom: 0,
                    })
                    .show(ui, |ui| {
                        self.add_connected_modulations(ui, &connected);
                    });
            }

            result_response
        })
        .inner
    }
}
