use std::any::Any;

use itertools::izip;
use serde::{Deserialize, Serialize};

use crate::synth_engine::{
    buffer::{
        HARMONIC_SERIES_BUFFER, SPECTRAL_BUFFER_SIZE, SpectralBuffer, make_harmonic_series_buffer,
        make_zero_spectral_buffer,
    },
    routing::{
        InputType, MAX_VOICES, ModuleId, ModuleInput, ModuleType, NUM_CHANNELS, OutputType, Router,
    },
    synth_module::{
        ModuleConfigBox, NoteOffParams, NoteOnParams, ProcessParams, ScalarOutputs,
        SpectralOutputs, SynthModule,
    },
    types::{ComplexSample, Sample, StereoSample},
};

pub const MAX_CUTOFF_HARMONIC: Sample = 1023.0;

static MODULE_INPUTS: &[InputType] = &[InputType::CutoffScalar];
static MODULE_OUTPUTS: &[OutputType] = &[OutputType::Spectrum];

#[derive(Clone, Serialize, Deserialize)]
pub struct SpectralFilterConfigChannel {
    cutoff: Sample,
}

impl Default for SpectralFilterConfigChannel {
    fn default() -> Self {
        Self {
            cutoff: MAX_CUTOFF_HARMONIC,
        }
    }
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct SpectralFilterConfig {
    channels: [SpectralFilterConfigChannel; NUM_CHANNELS],
}

struct Voice {
    needs_reset: bool,
    first_output: SpectralBuffer,
    output: SpectralBuffer,
}

impl Default for Voice {
    fn default() -> Self {
        Self {
            needs_reset: true,
            first_output: make_zero_spectral_buffer(),
            output: make_zero_spectral_buffer(),
        }
    }
}

struct ChannelParams {
    input_spectrum: SpectralBuffer,
    cutoff: Sample,
}

impl Default for ChannelParams {
    fn default() -> Self {
        Self {
            input_spectrum: make_harmonic_series_buffer(),
            cutoff: 10.0,
        }
    }
}

#[derive(Default)]
struct Channel {
    params: ChannelParams,
    voices: [Voice; MAX_VOICES],
}

pub struct SpectralFilter {
    id: ModuleId,
    config: ModuleConfigBox<SpectralFilterConfig>,
    channels: [Channel; NUM_CHANNELS],
}

impl SpectralFilter {
    pub fn new(id: ModuleId, config: ModuleConfigBox<SpectralFilterConfig>) -> Self {
        let mut filter = Self {
            id,
            config,
            channels: Default::default(),
        };

        {
            let cfg = filter.config.lock();
            for (channel, cfg_channel) in filter.channels.iter_mut().zip(cfg.channels.iter()) {
                channel.params.cutoff = cfg_channel.cutoff;
            }
        }

        filter
    }

    pub fn downcast(module: &dyn SynthModule) -> Option<&SpectralFilter> {
        (module as &dyn Any).downcast_ref()
    }

    pub fn downcast_mut(module: &mut dyn SynthModule) -> Option<&mut SpectralFilter> {
        (module as &mut dyn Any).downcast_mut()
    }

    pub fn set_cutoff_harmonic(&mut self, cutoff: StereoSample) {
        for (channel, cutoff) in self.channels.iter_mut().zip(cutoff.iter()) {
            channel.params.cutoff = cutoff.clamp(0.0, MAX_CUTOFF_HARMONIC);
        }

        {
            let mut cfg = self.config.lock();
            for (config_channel, channel) in cfg.channels.iter_mut().zip(self.channels.iter()) {
                config_channel.cutoff = channel.params.cutoff;
            }
        }
    }

    pub fn set_harmonics(&mut self, harmonics: &[StereoSample], tail: StereoSample) {
        let (channel_l, channel_r) = self.channels.split_at_mut(1);
        let buff_l = &mut channel_l[0].params.input_spectrum;
        let buff_r = &mut channel_r[0].params.input_spectrum;
        let range = 1..(harmonics.len() + 1);

        for ((out_l, out_r), series_factor, harmonic) in izip!(
            buff_l[range.clone()]
                .iter_mut()
                .zip(buff_r[range.clone()].iter_mut()),
            &HARMONIC_SERIES_BUFFER[range],
            harmonics
        ) {
            *out_l = series_factor * harmonic.left();
            *out_r = series_factor * harmonic.right();
        }

        let range = (harmonics.len() + 1)..buff_l.len();

        for ((out_l, out_r), series_factor) in buff_l[range.clone()]
            .iter_mut()
            .zip(buff_r[range.clone()].iter_mut())
            .zip(HARMONIC_SERIES_BUFFER[range].iter())
        {
            *out_l = series_factor * tail.left();
            *out_r = series_factor * tail.right();
        }

        buff_l[0] = ComplexSample::ZERO;
        buff_r[0] = ComplexSample::ZERO;
    }

    fn process_buffer(
        channel: &ChannelParams,
        output_buff: &mut SpectralBuffer,
        cutoff_mod: Sample,
    ) {
        let range = 1..SPECTRAL_BUFFER_SIZE - 1;
        let input_buff = &channel.input_spectrum[range.clone()];
        let output_buff = &mut output_buff[range];
        let cutoff = channel.cutoff + cutoff_mod;
        let cutoff_squared = cutoff * cutoff;
        let numerator = ComplexSample::new(cutoff_squared, 0.0);
        let q_mult: Sample = (0.7_f32).recip();

        for (idx, (out_freq, in_freq)) in output_buff.iter_mut().zip(input_buff).enumerate() {
            let freq = (idx + 1) as Sample;
            let filter_response = numerator
                / ComplexSample::new(cutoff_squared - (freq * freq), cutoff * freq * q_mult);

            *out_freq = filter_response * in_freq;
        }
    }

    fn process_channel_voice(
        module_id: ModuleId,
        channel: &mut Channel,
        router: &dyn Router,
        voice_idx: usize,
        channel_idx: usize,
    ) {
        let voice = &mut channel.voices[voice_idx];
        let cutoff_mod = router
            .get_scalar_input(
                ModuleInput::cutoff_scalar(module_id),
                voice_idx,
                channel_idx,
            )
            .unwrap_or(ScalarOutputs::zero());

        if voice.needs_reset {
            Self::process_buffer(&channel.params, &mut voice.first_output, cutoff_mod.first);
            voice.needs_reset = false;
        }

        Self::process_buffer(&channel.params, &mut voice.output, cutoff_mod.current);
    }
}

impl SynthModule for SpectralFilter {
    fn id(&self) -> ModuleId {
        self.id
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::SpectralFilter
    }

    fn inputs(&self) -> &'static [InputType] {
        MODULE_INPUTS
    }

    fn outputs(&self) -> &'static [OutputType] {
        MODULE_OUTPUTS
    }

    fn note_on(&mut self, params: &NoteOnParams) {
        if !params.same_note_retrigger {
            for channel in &mut self.channels {
                channel.voices[params.voice_idx].needs_reset = true;
            }
        }
    }

    fn note_off(&mut self, _: &NoteOffParams) {}

    fn process(&mut self, params: &ProcessParams, router: &dyn Router) {
        for (channel_idx, channel) in self.channels.iter_mut().enumerate() {
            for voice_idx in params.active_voices {
                Self::process_channel_voice(self.id, channel, router, *voice_idx, channel_idx);
            }
        }
    }

    fn get_spectral_output(&self, voice_idx: usize, channel: usize) -> SpectralOutputs<'_> {
        let voice = &self.channels[channel].voices[voice_idx];

        SpectralOutputs {
            first: &voice.first_output,
            current: &voice.output,
        }
    }

    fn get_buffer_output(
        &self,
        _voice_idx: usize,
        _channel: usize,
    ) -> &crate::synth_engine::buffer::Buffer {
        panic!("SpectralFilter don't have buffer output.")
    }

    fn get_scalar_output(&self, _voice_idx: usize, _channel: usize) -> ScalarOutputs {
        panic!("SpectralFilter don't have scalar output.")
    }
}
