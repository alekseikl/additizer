use std::any::Any;
use std::sync::Arc;

use itertools::izip;
use nih_plug::{params::FloatParam, util::db_to_gain_fast};
use serde::{Deserialize, Serialize};

use crate::{
    synth_engine::{
        Input, ModuleId, ModuleInput, ModuleType, OUTPUT_MODULE_ID, Sample, StereoSample,
        SynthModule,
        buffer::{Buffer, copy_or_add_to_buffer, copy_to_buffer, zero_buffer},
        iir_decimator::IirDecimator,
        routing::{DataType, MAX_VOICES, NUM_CHANNELS, Router, VoiceEvent},
        smooth::{InfiniteSmoothed, SmoothedSample},
        synth_module::{ModInput, ModuleConfigBox, ProcessParams},
        voices_handler::DecayingVoice,
    },
    utils::from_ms,
};

const _: () = assert!(NUM_CHANNELS == 2);

#[derive(Clone, Serialize, Deserialize)]
pub struct Params {
    gain: [SmoothedSample; NUM_CHANNELS],
    voice_kill_time: Sample,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            gain: [0.5.into(), 0.5.into()],
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
    ext_level_param: Arc<FloatParam>,
    ext_gain_smoothed: InfiniteSmoothed,
    channels: [Channel; NUM_CHANNELS],
    input_buffer: Buffer,
    ext_gain_buffer: Buffer,
    output: [Buffer; NUM_CHANNELS],
    decimator: IirDecimator,
}

impl Output {
    pub fn new(config: ModuleConfigBox<OutputConfig>, level_param: Arc<FloatParam>) -> Self {
        let ext_gain = db_to_gain_fast(level_param.value());

        let mut out = Self {
            params: Params::default(),
            config,
            ext_level_param: level_param,
            ext_gain_smoothed: ext_gain.into(),
            channels: Default::default(),
            input_buffer: zero_buffer(),
            ext_gain_buffer: zero_buffer(),
            output: [zero_buffer(), zero_buffer()],
            decimator: IirDecimator::new(),
        };

        out.params = out.config.lock().params.clone();

        out
    }

    gen_downcast_methods!();

    pub fn get_gain(&self) -> StereoSample {
        StereoSample::from_iter(self.params.gain.iter().map(|s| s.get()))
    }

    pub fn set_gain(&mut self, gain: StereoSample) {
        for (smoothed_gain, gain) in self.params.gain.iter_mut().zip(gain.iter()) {
            smoothed_gain.set(*gain);
        }

        self.config.lock().params.gain = self.params.gain;
    }

    pub fn get_voice_kill_time(&self) -> Sample {
        self.params.voice_kill_time
    }

    pub fn set_voice_kill_time(&mut self, voice_kill_time: Sample) {
        self.params.voice_kill_time = voice_kill_time;
        self.config.lock().params.voice_kill_time = voice_kill_time;
    }

    pub fn read_output<'a>(
        &mut self,
        oversampling: bool,
        mut outputs: impl Iterator<Item = &'a mut [f32]>,
    ) {
        if oversampling {
            self.decimator.process(
                [&self.output[0], &self.output[1]],
                [outputs.next().unwrap(), outputs.next().unwrap()],
            );
        } else {
            for (out, aggregated) in outputs.zip(self.output.iter()) {
                for (out, aggregated) in out.iter_mut().zip(aggregated.iter()) {
                    *out = *aggregated;
                }
            }
        }
    }
}

impl SynthModule for Output {
    fn id(&self) -> ModuleId {
        OUTPUT_MODULE_ID
    }

    fn label(&self) -> String {
        "Output".to_string()
    }

    fn set_label(&mut self, _label: String) {}

    fn module_type(&self) -> ModuleType {
        ModuleType::Output
    }

    fn inputs(&self) -> &'static [ModInput] {
        static INPUTS: &[ModInput] = &[ModInput::buffer(Input::Audio)];

        INPUTS
    }

    fn output(&self) -> DataType {
        DataType::Buffer
    }

    fn handle_events(&mut self, events: &[VoiceEvent]) {
        for channel in &mut self.channels {
            for event in events {
                match event {
                    VoiceEvent::Trigger { voice_idx, .. } => {
                        let voice = &mut channel.voices[*voice_idx];

                        voice.killed = false;
                        voice.killed_gain = 1.0;
                        voice.killed_output_power = 1.0;
                    }
                    VoiceEvent::Kill { voice_idx } => {
                        channel.voices[*voice_idx].killed = true;
                    }
                    _ => (),
                }
            }
        }
    }

    fn poll_decaying_voices(&self, decaying_voices: &mut [DecayingVoice]) {
        const ALIVE_THRESHOLD: Sample = 0.00001;

        for decaying in decaying_voices.iter_mut().filter(|d| !d.is_done()) {
            decaying.reset();

            for channel in &self.channels {
                let voice = &channel.voices[decaying.index()];

                if !voice.killed || voice.killed_output_power > ALIVE_THRESHOLD {
                    decaying.mark_active();
                }
            }
        }
    }

    fn process(&mut self, process_params: &ProcessParams, router: &dyn Router) {
        if process_params.active_voices.is_empty() {
            self.output.iter_mut().for_each(|output| output.fill(0.0));
            return;
        }

        let sample_rate = process_params.sample_rate;
        let samples = process_params.samples;

        self.ext_gain_smoothed
            .set(db_to_gain_fast(self.ext_level_param.value()));

        copy_to_buffer(
            &mut self.ext_gain_buffer,
            self.ext_gain_smoothed
                .iter(InfiniteSmoothed::smooth_mult(sample_rate, from_ms(4.0)))
                .take(samples),
        );

        for (channel_idx, (output, gain)) in self
            .output
            .iter_mut()
            .zip(self.params.gain.iter_mut())
            .enumerate()
        {
            for (iteration_idx, voice_idx) in process_params.active_voices.iter().enumerate() {
                router.read_unmodulated_input(
                    ModuleInput::new(Input::Audio, OUTPUT_MODULE_ID),
                    samples,
                    *voice_idx,
                    channel_idx,
                    &mut self.input_buffer,
                );

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

                copy_or_add_to_buffer(
                    iteration_idx == 0,
                    output,
                    self.input_buffer.iter().copied().take(samples),
                );
            }

            fn apply_volume<'a>(
                output: impl Iterator<Item = &'a mut Sample>,
                gain: impl Iterator<Item = Sample>,
                ext_gain: impl Iterator<Item = &'a Sample>,
                samples: usize,
            ) {
                for (out, gain, gain_ext) in izip!(output, gain, ext_gain).take(samples) {
                    *out *= gain * gain_ext;
                }
            }

            if gain.check_needs_smoothing(&process_params.smooth_params) {
                apply_volume(
                    output.iter_mut(),
                    gain.smoothed_iter(&process_params.smooth_params),
                    self.ext_gain_buffer.iter(),
                    samples,
                );
            } else {
                apply_volume(
                    output.iter_mut(),
                    std::iter::repeat(gain.get()),
                    self.ext_gain_buffer.iter(),
                    samples,
                );
            }
        }
    }
}
