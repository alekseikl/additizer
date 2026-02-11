use std::sync::Arc;

use nih_plug::{params::FloatParam, util::db_to_gain_fast};
use serde::{Deserialize, Serialize};

use crate::{
    synth_engine::{
        Input, ModuleInput, OUTPUT_MODULE_ID, Sample, StereoSample,
        buffer::{Buffer, ZEROES_BUFFER, copy_or_add_buffer, zero_buffer},
        iir_decimator::IirDecimator,
        routing::{MAX_VOICES, NUM_CHANNELS, Router},
        smoother::Smoother,
        synth_module::{ModuleConfigBox, NoteOnParams, ProcessParams, VoiceAlive},
        types::ScalarOutput,
    },
    utils::from_ms,
};

const _: () = assert!(NUM_CHANNELS == 2);

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
    aggregated_voices: [Buffer; NUM_CHANNELS],
    decimator: IirDecimator,
    level_mod_output: ScalarOutput,
    level_mod_smoother: Smoother,
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
            aggregated_voices: [zero_buffer(), zero_buffer()],
            decimator: IirDecimator::new(),
            level_mod_output: ScalarOutput::default(),
            level_mod_smoother: Smoother::new(),
        };

        let gain = db_to_gain_fast(out.output_level_param.value());

        out.params = out.config.lock().params.clone();
        out.level_mod_output.advance(gain);
        out.level_mod_smoother.reset(gain);

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
        oversampling: bool,
        router: &dyn Router,
        mut outputs: impl Iterator<Item = &'a mut [f32]>,
    ) {
        self.level_mod_output
            .advance(db_to_gain_fast(self.output_level_param.value()));

        if process_params.active_voices.is_empty() {
            if oversampling {
                self.decimator.process(
                    [&ZEROES_BUFFER, &ZEROES_BUFFER],
                    [outputs.next().unwrap(), outputs.next().unwrap()],
                );
            } else {
                outputs.for_each(|output| output.fill(0.0));
            }

            return;
        }

        let sample_rate = process_params.sample_rate;
        let samples = process_params.samples;

        self.level_mod_smoother.update(sample_rate, from_ms(4.0));
        self.level_mod_smoother.segment(
            &self.level_mod_output,
            samples,
            &mut self.gain_param_buffer,
        );

        for (channel_idx, (aggregated, gain)) in self
            .aggregated_voices
            .iter_mut()
            .zip(self.params.gain.iter())
            .enumerate()
        {
            for (iteration_idx, voice_idx) in process_params.active_voices.iter().enumerate() {
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

                    copy_or_add_buffer(
                        iteration_idx == 0,
                        aggregated,
                        self.input_buffer.iter().copied().take(samples),
                    );
                }
            }

            for (aggregated, gain_mod) in aggregated
                .iter_mut()
                .zip(self.gain_param_buffer.iter())
                .take(samples)
            {
                *aggregated *= gain_mod * gain;
            }
        }

        if oversampling {
            self.decimator.process(
                [&self.aggregated_voices[0], &self.aggregated_voices[1]],
                [outputs.next().unwrap(), outputs.next().unwrap()],
            );
        } else {
            for (out, aggregated) in outputs.zip(self.aggregated_voices.iter()) {
                for (out, aggregated) in out.iter_mut().zip(aggregated.iter()) {
                    *out = *aggregated;
                }
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
