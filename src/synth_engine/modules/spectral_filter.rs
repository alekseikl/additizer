use itertools::izip;

use crate::synth_engine::{
    buffer::{
        HARMONIC_SERIES_BUFFER, SPECTRAL_BUFFER_SIZE, SpectralBuffer, make_harmonic_series_buffer,
        make_zero_spectral_buffer,
    },
    routing::{MAX_VOICES, ModuleId, NUM_CHANNELS, Router},
    synth_module::{NoteOffParams, NoteOnParams, ProcessParams, SpectralOutputModule, SynthModule},
    types::{ComplexSample, Sample, StereoValue},
};

pub const MAX_CUTOFF_HARMONIC: Sample = 1023.0;

struct Voice {
    output: SpectralBuffer,
}

impl Default for Voice {
    fn default() -> Self {
        Self {
            output: make_zero_spectral_buffer(),
        }
    }
}

struct Channel {
    input_spectrum: SpectralBuffer,
    cutoff: Sample,
    voices: [Voice; MAX_VOICES],
}

impl Default for Channel {
    fn default() -> Self {
        Self {
            input_spectrum: make_harmonic_series_buffer(),
            cutoff: 10.0,
            voices: Default::default(),
        }
    }
}

pub struct SpectralFilter {
    module_id: ModuleId,
    channels: [Channel; NUM_CHANNELS],
}

impl SpectralFilter {
    pub fn new() -> Self {
        Self {
            module_id: 0,
            channels: Default::default(),
        }
    }

    pub fn set_id(&mut self, module_id: ModuleId) {
        self.module_id = module_id;
    }

    pub fn set_cutoff_harmonic(&mut self, cutoff: StereoValue) {
        for (channel, cutoff) in self.channels.iter_mut().zip(cutoff.iter()) {
            channel.cutoff = cutoff.clamp(0.0, MAX_CUTOFF_HARMONIC);
        }
    }

    pub fn set_harmonics(&mut self, harmonics: &[Sample], tail: Sample) {
        let buff = &mut self.channels[0].input_spectrum;
        let range = 1..(harmonics.len() + 1);

        for (out, series_factor, harmonic) in izip!(
            &mut buff[range.clone()],
            &HARMONIC_SERIES_BUFFER[range],
            harmonics
        ) {
            *out = series_factor * harmonic;
        }

        let range = (harmonics.len() + 1)..buff.len();

        for (out, series_factor) in buff[range.clone()]
            .iter_mut()
            .zip(HARMONIC_SERIES_BUFFER[range].iter())
        {
            *out = *series_factor * tail;
        }

        buff[0] = ComplexSample::ZERO;
        self.channels[1].input_spectrum = *buff;
    }

    fn process_channel_voice(channel: &mut Channel, voice_idx: usize, _channel_idx: usize) {
        let range = 1..SPECTRAL_BUFFER_SIZE - 1;
        let input_buff = &channel.input_spectrum[range.clone()];
        let output_buff = &mut channel.voices[voice_idx].output[range];
        let cutoff = channel.cutoff;
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
}

impl SynthModule for SpectralFilter {
    fn get_id(&self) -> ModuleId {
        self.module_id
    }

    fn note_on(&mut self, _: &NoteOnParams) {}
    fn note_off(&mut self, _: &NoteOffParams) {}

    fn process(&mut self, params: &ProcessParams, _router: &dyn Router) {
        for (channel_idx, channel) in self.channels.iter_mut().enumerate() {
            for voice_idx in params.active_voices {
                Self::process_channel_voice(channel, *voice_idx, channel_idx);
            }
        }
    }
}

impl SpectralOutputModule for SpectralFilter {
    fn get_output(&self, voice_idx: usize, channel: usize) -> &SpectralBuffer {
        &self.channels[channel].voices[voice_idx].output
    }
}
