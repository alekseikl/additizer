use crate::synth_engine::pitch::{Pitch, PitchConfig, PitchLinks, PitchUiEnd};
use crate::synth_engine::{Input, Sample, StereoSample, synth_module::ModuleUiBridge};

pub struct PitchUiBridge<L: PitchLinks = crate::links::pitch::Links> {
    ui_end: L::UiEnd,
    config: PitchConfig,
}

impl<L: PitchLinks> PitchUiBridge<L> {
    pub fn try_new(pitch: &mut Pitch<L>) -> Option<Self> {
        Some(Self {
            ui_end: pitch.take_ui_end()?,
            config: pitch.get_config(),
        })
    }

    pub fn config(&self) -> &PitchConfig {
        &self.config
    }

    pub fn get_pitch(&mut self) -> Sample {
        self.ui_end.get_pitch()
    }

    pub fn set_param(&mut self, input: Input, value: StereoSample) {
        if !self.ui_end.set_param(input, value) {
            return;
        }

        match input {
            Input::PitchShift => self.config.pitch_shift = value,
            Input::Glide => self.config.glide = value,
            Input::GlideSlope => self.config.glide_slope = value,
            _ => (),
        }
    }

    pub fn set_keytrack(&mut self, value: bool) {
        if self.ui_end.set_keytrack(value) {
            self.config.keytrack = value;
        }
    }

    pub fn set_glide_always(&mut self, value: bool) {
        if self.ui_end.set_glide_always(value) {
            self.config.glide_always = value;
        }
    }

    pub fn set_glide_per_octave(&mut self, value: bool) {
        if self.ui_end.set_glide_per_octave(value) {
            self.config.glide_per_octave = value;
        }
    }
}

impl<L: PitchLinks> ModuleUiBridge for PitchUiBridge<L> {
    fn update(&mut self) -> bool {
        false
    }
}
