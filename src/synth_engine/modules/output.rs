use std::sync::Arc;

use nih_plug::{params::FloatParam, util::db_to_gain_fast};
use serde::{Deserialize, Serialize};

use crate::{
    synth_engine::{
        Input, ModuleInput, OUTPUT_MODULE_ID, Sample, StereoSample,
        buffer::{Buffer, zero_buffer},
        routing::{MAX_VOICES, NUM_CHANNELS, Router},
        synth_module::{ModuleConfigBox, NoteOnParams, ProcessParams, VoiceAlive},
    },
    utils::from_ms,
};

#[derive(Clone, Serialize, Deserialize)]
pub struct Params {
    gain: StereoSample,
    voice_kill_time: Sample,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            gain: StereoSample::splat(0.5),
            voice_kill_time: from_ms(30.0),
        }
    }
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct OutputConfig {
    params: Params,
}

struct Voice {
    killed: bool,
    killed_output_power: Sample,
    killed_gain: Sample,
}

impl Default for Voice {
    fn default() -> Self {
        Self {
            killed: false,
            killed_gain: 0.0,
            killed_output_power: 0.0,
        }
    }
}

#[derive(Default)]
struct Channel {
    voices: [Voice; MAX_VOICES],
}

pub struct Output {
    config: ModuleConfigBox<OutputConfig>,
    params: Params,
    output_level_param: Arc<FloatParam>,
    channels: [Channel; NUM_CHANNELS],
    gain_param_buffer: Buffer,
    input_buffer: Buffer,
}

impl Output {
    pub fn new(config: ModuleConfigBox<OutputConfig>, level_param: Arc<FloatParam>) -> Self {
        let mut out = Self {
            params: Params::default(),
            config,
            output_level_param: level_param,
            channels: Default::default(),
            gain_param_buffer: zero_buffer(),
            input_buffer: zero_buffer(),
        };

        out.params = out.config.lock().params.clone();
        out
    }

    pub fn get_gain(&self) -> StereoSample {
        self.params.gain
    }

    pub fn set_gain(&mut self, gain: StereoSample) {
        self.params.gain = gain;
        self.config.lock().params.gain = gain;
    }

    pub fn get_voice_kill_time(&self) -> Sample {
        self.params.voice_kill_time
    }

    pub fn set_voice_kill_time(&mut self, voice_kill_time: Sample) {
        self.params.voice_kill_time = voice_kill_time;
        self.config.lock().params.voice_kill_time = voice_kill_time;
    }

    pub fn process<'a>(
        &mut self,
        process_params: &ProcessParams,
        router: &dyn Router,
        outputs: impl Iterator<Item = &'a mut [f32]>,
    ) {
        let samples = process_params.samples;
        let sample_rate = process_params.sample_rate;

        self.output_level_param.smoothed.next_block_mapped(
            &mut self.gain_param_buffer,
            samples,
            |_, dbs| db_to_gain_fast(dbs),
        );

        for (channel_idx, (output, gain)) in outputs.zip(self.params.gain.iter()).enumerate() {
            output.fill(0.0);

            let output = &mut output[..samples];

            for voice_idx in process_params.active_voices.iter() {
                if router.read_unmodulated_input(
                    ModuleInput::new(Input::Audio, OUTPUT_MODULE_ID),
                    samples,
                    *voice_idx,
                    channel_idx,
                    &mut self.input_buffer,
                ) {
                    let voice = &mut self.channels[channel_idx].voices[*voice_idx];

                    if voice.killed {
                        let kill_time = self.params.voice_kill_time.max(from_ms(4.0));
                        let base = (-5.0 / (sample_rate * kill_time)).exp();
                        let mut sum = 0.0;

                        for out in self.input_buffer.iter_mut().take(samples) {
                            voice.killed_gain *= base;
                            *out *= voice.killed_gain;
                            sum += *out * *out;
                        }

                        voice.killed_output_power =
                            (voice.killed_output_power + sum) / (samples + 1) as Sample;
                    }

                    output
                        .iter_mut()
                        .zip(&self.input_buffer)
                        .for_each(|(out, value)| *out += value);
                }
            }

            for (out, gain_mod) in output.iter_mut().zip(self.gain_param_buffer.iter()) {
                *out *= gain_mod * gain;
            }
        }
    }

    pub fn note_on(&mut self, params: &NoteOnParams) {
        for channel in &mut self.channels {
            let voice = &mut channel.voices[params.voice_idx];

            voice.killed = false;
            voice.killed_gain = 1.0;
            voice.killed_output_power = 1.0;
        }
    }

    pub fn kill_voice(&mut self, voice_idx: usize) {
        for channel in &mut self.channels {
            channel.voices[voice_idx].killed = true;
        }
    }

    pub fn poll_alive_voices(&self, alive_state: &mut [VoiceAlive]) {
        const ALIVE_THRESHOLD: Sample = 0.0000001;

        for voice_alive in alive_state.iter_mut().filter(|alive| alive.alive()) {
            for channel in &self.channels {
                let voice = &channel.voices[voice_alive.index()];

                if voice.killed {
                    voice_alive.reset_alive(voice.killed_output_power > ALIVE_THRESHOLD);
                }
            }
        }
    }
}
