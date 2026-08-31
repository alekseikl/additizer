use nice_plug::util::db_to_gain_fast;

use crate::{
    synth_engine::{
        Input, ModuleId, OUTPUT_MODULE_ID, Sample, StereoSample, SynthModule,
        buffer::{Buffer, copy_or_add_to_buffer, copy_to_buffer, zero_buffer},
        iir_decimator::IirDecimator,
        routing::{
            DataType, InputMeta, InputSlots, MAX_VOICES, NUM_CHANNELS, ProcessContext,
            SpectralInputSlot, VoiceEvent, VoiceTarget,
        },
        smooth::SmoothedSample,
        voices_handler::DecayingVoice,
    },
    utils::from_ms,
};

const _: () = assert!(NUM_CHANNELS == 2);

/// Hard-clip ceiling for the summed output.
const OUTPUT_CLIP_DB: Sample = 12.0;

struct Voice {
    killing: bool,
    /// In-block sample index where the kill fade starts; taken on the next process.
    killing_offset: Option<usize>,
    killing_time: Sample,
}

impl Default for Voice {
    fn default() -> Self {
        Self {
            killing: false,
            killing_offset: None,
            killing_time: 0.0,
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
    channels: [Channel; NUM_CHANNELS],
    input_buffer: Buffer,
    output: [Buffer; NUM_CHANNELS],
    decimator: IirDecimator,
}

impl Output {
    pub fn new(gain: StereoSample, kill_time: Sample) -> Self {
        Self {
            audio_input: None,
            gain: [
                SmoothedSample::new(Self::clamp_gain(gain[0])),
                SmoothedSample::new(Self::clamp_gain(gain[1])),
            ],
            kill_time: Self::clamp_kill_time(kill_time),
            channels: Default::default(),
            input_buffer: zero_buffer(),
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

    pub fn read_output(&mut self, oversampling: bool, outputs: &mut [&mut [f32]; NUM_CHANNELS]) {
        if oversampling {
            let (left, right) = outputs.split_at_mut(1);
            self.decimator
                .process([&self.output[0], &self.output[1]], [left[0], right[0]]);
        } else {
            for (out, aggregated) in outputs.iter_mut().zip(self.output.iter()) {
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

    fn inputs(&self) -> &'static [InputMeta] {
        static INPUTS: &[InputMeta] = &[InputMeta::direct_audio(Input::Audio)];

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

    fn process_events(&mut self, events: &[VoiceEvent]) {
        for channel in &mut self.channels {
            for event in events {
                match event {
                    VoiceEvent::Reset { voice_idx, .. } => {
                        let voice = &mut channel.voices[*voice_idx];

                        voice.killing = false;
                        voice.killing_offset = None;
                        voice.killing_time = 0.0;
                    }
                    VoiceEvent::Kill { voice_idx, offset } => {
                        let voice = &mut channel.voices[*voice_idx];

                        voice.killing = true;
                        voice.killing_offset = Some(*offset);
                    }
                    _ => (),
                }
            }
        }
    }

    fn poll_decaying_voices(&self, decaying_voices: &mut [DecayingVoice]) {
        for decaying in decaying_voices.iter_mut().filter(|d| !d.is_done()) {
            decaying.reset();

            for channel in &self.channels {
                let voice = &channel.voices[decaying.index()];

                if !voice.killing || voice.killing_time < self.kill_time {
                    decaying.mark_active();
                }
            }
        }
    }

    fn process_ui_events(&mut self) {}

    fn process(&mut self, ctx: &mut ProcessContext) {
        if ctx.params.trigger_stage {
            return;
        }

        let mut rf = ctx.for_output(self.id());
        let num_active_voices = rf.params().active_voices.len();

        if num_active_voices == 0 {
            self.output.iter_mut().for_each(|output| output.fill(0.0));
            // Advance gain parameter smoother
            self.gain
                .iter_mut()
                .for_each(|gain| gain.advance(&rf.params().smooth_params, rf.params().samples));
            return;
        }

        let sample_rate = rf.params().sample_rate;
        let samples = rf.params().samples;

        for (channel_idx, (output, gain)) in
            self.output.iter_mut().zip(self.gain.iter_mut()).enumerate()
        {
            for seq_idx in 0..num_active_voices {
                let playing = rf.params().active_voices[seq_idx];
                let target = VoiceTarget::new(channel_idx, &playing, seq_idx);
                let mut router = rf.for_voice(&target);

                copy_to_buffer(
                    &mut self.input_buffer[..samples],
                    router.direct(self.audio_input).iter().copied(),
                );

                let voice = &mut self.channels[channel_idx].voices[target.voice_idx];

                if voice.killing {
                    let start = voice.killing_offset.take().unwrap_or(0).min(samples);
                    let power: Sample = -5.0;
                    let curve_mult: Sample = (power.exp() - 1.0).recip();
                    let time_mult: Sample = self.kill_time.max(from_ms(4.0)).recip();
                    let t_step = sample_rate.recip();

                    for out in self.input_buffer[..samples].iter_mut().skip(start) {
                        let t = (voice.killing_time * time_mult).min(1.0);
                        let gain = 1.0 - ((power * t).exp() - 1.0) * curve_mult;

                        *out *= gain;
                        voice.killing_time += t_step;
                    }
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
                samples: usize,
            ) {
                let clip = db_to_gain_fast(OUTPUT_CLIP_DB);

                for (out, gain) in output.zip(gain).take(samples) {
                    *out = (*out * gain).clamp(-clip, clip);
                }
            }

            if gain.check_needs_smoothing(&rf.params().smooth_params) {
                apply_volume(
                    output.iter_mut(),
                    gain.smoothed_iter(&rf.params().smooth_params),
                    samples,
                );
            } else {
                apply_volume(output.iter_mut(), std::iter::repeat(gain.get()), samples);
            }

            // Advance gain parameter smoother
            gain.advance(&rf.params().smooth_params, rf.params().samples);
        }
    }
}
