use std::array;

use crate::synth_engine::{SmoothedSampleParams, smooth::SmoothedSample, types::Smp};

use super::{MAX_UNISON_VOICES, OscillatorConfig, UnisonStyle};

pub struct Params {
    pub unison: usize,
    pub unison_style: UnisonStyle,
    pub unison_stereo: Smp, // [0.0, 1.0]
    pub steal_phase: bool,
    pub phase_random: Smp,        // [0.0, 1.0]
    pub phase_random_stereo: Smp, // [0.0, 1.0]
    pub mono_spectrum: bool,
    pub full_fft_size: bool,
}

impl Params {
    pub fn from_config(c: &OscillatorConfig) -> Self {
        Self {
            unison: c.unison_voices,
            unison_style: c.unison_style,
            unison_stereo: c.unison_stereo,
            steal_phase: c.steal_phase,
            phase_random: c.phase_random,
            phase_random_stereo: c.phase_random_stereo,
            mono_spectrum: c.mono_spectrum,
            full_fft_size: c.full_fft_size,
        }
    }
}

pub struct UnisonParams {
    pub initial_phase: Smp,
    pub phase_shift: Smp,
    pub phase_shift_to: Smp,
    /// Hand-set level. Read by [`UnisonStyle::Manual`] only.
    pub gain: Smp,
    /// Hand-set level at full gains blend. Read by [`UnisonStyle::Manual`] only.
    pub gain_to: Smp,
}

impl Default for UnisonParams {
    fn default() -> Self {
        Self {
            initial_phase: 0.0,
            phase_shift: 0.0,
            phase_shift_to: 0.0,
            gain: 1.0,
            gain_to: 1.0,
        }
    }
}

pub struct ChannelParams {
    pub detune: Smp, // Octaves
    pub detune_focus: Smp,
    pub phase_shift: SmoothedSample,
    pub frequency_shift: SmoothedSample,
    pub phases_blend: Smp,
    pub gains_blend: Smp,
    pub unison: [UnisonParams; MAX_UNISON_VOICES],
}

impl ChannelParams {
    pub fn from_config(c: &OscillatorConfig, channel_idx: usize) -> Self {
        Self {
            detune: c.detune[channel_idx],
            detune_focus: c.detune_focus[channel_idx],
            phase_shift: c.phase_shift[channel_idx].into(),
            frequency_shift: c.frequency_shift[channel_idx].into(),
            phases_blend: c.phases_blend[channel_idx],
            gains_blend: c.gains_blend[channel_idx],
            unison: array::from_fn(|i| UnisonParams {
                initial_phase: c.unison[i].initial_phase[channel_idx],
                phase_shift: c.unison[i].phase_shift[channel_idx],
                phase_shift_to: c.unison[i].phase_shift_to[channel_idx],
                gain: c.unison[i].gain[channel_idx],
                gain_to: c.unison[i].gain_to[channel_idx],
            }),
        }
    }

    pub fn advance_smoothers(&mut self, smooth_params: &SmoothedSampleParams, samples: usize) {
        self.phase_shift.advance(smooth_params, samples);
        self.frequency_shift.advance(smooth_params, samples);
    }
}
