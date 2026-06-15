use std::sync::Arc;

use itertools::izip;
use nih_plug::{params::FloatParam, util::db_to_gain_fast};

use crate::{
    synth_engine::{
        Input, ModuleId, OUTPUT_MODULE_ID, Sample, StereoSample, SynthModule,
        buffer::{Buffer, copy_or_add_to_buffer, copy_to_buffer, zero_buffer},
        iir_decimator::IirDecimator,
        routing::{
            DataType, InputSlots, MAX_VOICES, NUM_CHANNELS, ProcessContext, SpectralInputSlot,
            VoiceEvent,
        },
        smooth::{InfiniteSmoothed, SmoothedSample},
        synth_module::ModInput,
        voices_handler::DecayingVoice,
    },
    utils::from_ms,
};

const _: () = assert!(NUM_CHANNELS == 2);

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
    audio_input: Option<usize>,
    gain: [SmoothedSample; NUM_CHANNELS],
    kill_time: Sample,
    ext_level_param: Arc<FloatParam>,
    ext_gain_smoothed: InfiniteSmoothed,
    channels: [Channel; NUM_CHANNELS],
    input_buffer: Buffer,
    ext_gain_buffer: Buffer,
    output: [Buffer; NUM_CHANNELS],
    decimator: IirDecimator,
}

impl Output {
    pub fn new(gain: StereoSample, kill_time: Sample, level_param: Arc<FloatParam>) -> Self {
        let ext_gain = db_to_gain_fast(level_param.value());

        Self {
            audio_input: None,
            gain: [
                SmoothedSample::new(Self::clamp_gain(gain[0])),
                SmoothedSample::new(Self::clamp_gain(gain[1])),
            ],
            kill_time: Self::clamp_kill_time(kill_time),
            ext_level_param: level_param,
            ext_gain_smoothed: ext_gain.into(),
            channels: Default::default(),
            input_buffer: zero_buffer(),
            ext_gain_buffer: zero_buffer(),
            output: [zero_buffer(), zero_buffer()],
            decimator: IirDecimator::new(),
        }
    }

    fn clamp_kill_time(kill_time: Sample) -> Sample {
        kill_time.clamp(from_ms(4.0), from_ms(50.0))
    }

    fn clamp_gain(gain: Sample) -> Sample {
        gain.clamp(0.0, 4.0)
    }

    pub fn get_gain(&self) -> StereoSample {
        StereoSample::from_iter(self.gain.iter().map(|s| s.get()))
    }

    pub fn set_gain(&mut self, gain: StereoSample) {
        for (smoothed_gain, gain) in self.gain.iter_mut().zip(gain.iter()) {
            smoothed_gain.set(Self::clamp_gain(*gain));
        }
    }

    pub fn get_voice_kill_time(&self) -> Sample {
        self.kill_time
    }

    pub fn set_voice_kill_time(&mut self, kill_time: Sample) {
        self.kill_time = Self::clamp_kill_time(kill_time)
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

    fn inputs(&self) -> &'static [ModInput] {
        static INPUTS: &[ModInput] = &[ModInput::audio(Input::Audio)];

        INPUTS
    }

    fn output_type(&self) -> DataType {
        DataType::Audio
    }

    fn output_slot(&self) -> usize {
        usize::MAX
    }

    fn set_output_slot(&mut self, _slot: usize) {
        panic!("Output module doesn't have output slot.")
    }

    fn set_input_slots(&mut self, inputs: &[InputSlots], _spectral_inputs: &[SpectralInputSlot]) {
        self.audio_input = inputs.first().and_then(|s| s.first_slot());
    }

    fn update_input_amount(&mut self, _input_type: Input, _src_slot: usize, _amount: StereoSample) {
    }

    fn process_events(&mut self, events: &[VoiceEvent]) {
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
        const ALIVE_THRESHOLD: Sample = 0.000001;

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

    fn process_ui_events(&mut self) {}

    fn process(&mut self, ctx: &mut ProcessContext) {
        let mut rf = ctx.for_output(self.id());
        let num_active_voices = rf.params().active_voices.len();

        if num_active_voices == 0 {
            self.output.iter_mut().for_each(|output| output.fill(0.0));
            return;
        }

        let sample_rate = rf.params().sample_rate;
        let samples = rf.params().samples;

        self.ext_gain_smoothed
            .set(db_to_gain_fast(self.ext_level_param.value()));

        copy_to_buffer(
            &mut self.ext_gain_buffer,
            self.ext_gain_smoothed
                .iter(InfiniteSmoothed::smooth_mult(sample_rate, from_ms(4.0)))
                .take(samples),
        );

        for (channel_idx, (output, gain)) in
            self.output.iter_mut().zip(self.gain.iter_mut()).enumerate()
        {
            for seq_idx in 0..num_active_voices {
                let voice_idx = rf.params().active_voices[seq_idx];
                let mut router = rf.for_voice(channel_idx, voice_idx, seq_idx);

                copy_to_buffer(
                    &mut self.input_buffer[..samples],
                    router.buff(self.audio_input).iter().copied(),
                );

                let voice = &mut self.channels[channel_idx].voices[voice_idx];

                if voice.killed {
                    let kill_time = self.kill_time.max(from_ms(4.0));
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
                    seq_idx == 0,
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

            if gain.check_needs_smoothing(&rf.params().smooth_params) {
                apply_volume(
                    output.iter_mut(),
                    gain.smoothed_iter(&rf.params().smooth_params),
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
