use std::array;

use itertools::izip;

mod config;
mod link;
pub mod stub;

#[cfg(test)]
pub(crate) mod tests;

pub use config::AmplifierConfig;
pub use link::{AmplifierAudioEnd, AmplifierLinks, AmplifierUiEnd, UiEvent};

use crate::{
    synth_engine::{
        Sample, SmoothedSampleParams, StereoSample,
        buffer::{Buffer, VoicesLayout, zero_buffer},
        level_ballistics::LevelBallistics,
        routing::{
            AudioRouterType, DataType, Input, InputMeta, InputSlots, MixedSlots, ModuleId,
            NUM_CHANNELS, ProcessContext, RouterFactory, SamplesOutput, VoiceTarget,
        },
        smooth::SmoothedSample,
        synth_module::SynthModule,
    },
    synth_engine::{db_to_gain_fast, pan_gain},
};

const MAX_LEVEL_DB: Sample = 6.0;

struct ChannelParams {
    gain: SmoothedSample,
    level: SmoothedSample,
    pan: SmoothedSample,
}

impl ChannelParams {
    fn from_config(c: &AmplifierConfig, channel_idx: usize) -> Self {
        Self {
            gain: c.gain[channel_idx].into(),
            level: c.level[channel_idx].into(),
            pan: c.pan[channel_idx].into(),
        }
    }

    pub fn advance_smoothers(&mut self, smooth_params: &SmoothedSampleParams, samples: usize) {
        self.gain.advance(smooth_params, samples);
        self.level.advance(smooth_params, samples);
        self.pan.advance(smooth_params, samples);
    }
}

pub struct Inputs {
    audio: Option<usize>,
    gain: MixedSlots,
    level: MixedSlots,
    pan: MixedSlots,
}

impl Default for Inputs {
    fn default() -> Self {
        Self {
            audio: None,
            gain: MixedSlots::new(Input::Gain),
            level: MixedSlots::new(Input::Level),
            pan: MixedSlots::new(Input::Pan),
        }
    }
}

impl Inputs {
    fn from_slots(inputs: &[InputSlots]) -> Self {
        let mut result = Self::default();

        for input in inputs {
            match input {
                InputSlots::Direct { input_type, slot } => {
                    if matches!(input_type, Input::Audio) {
                        result.audio = Some(*slot);
                    }
                }
                InputSlots::Mixed(input) => match input.input_type {
                    Input::Gain => result.gain = input.clone(),
                    Input::Level => result.level = input.clone(),
                    Input::Pan => result.pan = input.clone(),
                    _ => (),
                },
                InputSlots::Spectral { .. } => (),
            }
        }

        result
    }

    fn update_amount(&mut self, input_type: Input, src_slot: usize, amount: StereoSample) {
        match input_type {
            Input::Gain => self.gain.update_amount(src_slot, amount),
            Input::Level => self.level.update_amount(src_slot, amount),
            Input::Pan => self.pan.update_amount(src_slot, amount),
            _ => (),
        }
    }
}

struct Buffers {
    gain_mod_input: Buffer,
}

pub struct Amplifier<L: AmplifierLinks = stub::Links> {
    id: ModuleId,
    channel_params: [ChannelParams; NUM_CHANNELS],
    buffers: Buffers,
    audio_end: L::AudioEnd,
    ui_end: Option<L::UiEnd>,
    inputs: Inputs,
    output_slot: usize,
    out_volume_ballistics: [LevelBallistics; NUM_CHANNELS],
}

impl<L: AmplifierLinks> Amplifier<L> {
    pub fn take_ui_end(&mut self) -> Option<L::UiEnd> {
        self.ui_end.take()
    }

    pub fn new(id: ModuleId) -> Self {
        Self::from_config(&AmplifierConfig {
            id,
            ..AmplifierConfig::default()
        })
    }

    pub fn from_config(config: &config::AmplifierConfig) -> Self {
        let (audio_end, ui_end) = L::create_link_pair();

        Self {
            id: config.id,
            channel_params: array::from_fn(|channel_idx| {
                ChannelParams::from_config(config, channel_idx)
            }),
            buffers: Buffers {
                gain_mod_input: zero_buffer(),
            },
            audio_end,
            ui_end,
            inputs: Inputs::default(),
            output_slot: usize::MAX,
            out_volume_ballistics: [LevelBallistics::default(); NUM_CHANNELS],
        }
    }

    pub fn get_config(&self) -> AmplifierConfig {
        AmplifierConfig {
            id: self.id,
            gain: get_smoothed_param!(self, gain),
            level: get_smoothed_param!(self, level),
            pan: get_smoothed_param!(self, pan),
        }
    }

    set_smoothed_param!(set_gain, gain);
    set_smoothed_param!(set_level, level);
    set_smoothed_param!(set_pan, pan, pan.clamp(-1.0, 1.0));

    fn process_voice(
        &mut self,
        target: &VoiceTarget,
        outputs: &mut VoicesLayout<SamplesOutput>,
        rf: &mut RouterFactory<AudioRouterType, L::EngineEnd>,
    ) {
        let (mut router, mut voice_output) = rf.for_voice(target, outputs);
        let inputs = &self.inputs;
        let channel = &mut self.channel_params[target.channel_idx];
        let channel_idx = target.channel_idx;

        router.param(
            &inputs.gain,
            &channel.gain,
            &mut self.buffers.gain_mod_input,
        );

        let input = router.direct(inputs.audio);
        let output = voice_output.output();

        for (out, input, modulation) in
            izip!(output.iter_mut(), input, &self.buffers.gain_mod_input)
        {
            *out = input * modulation;
        }

        if let Some(level) = router.param_stationary(&inputs.level, &channel.level) {
            if level.abs() >= 1e-6 {
                let gain = db_to_gain_fast(level.min(MAX_LEVEL_DB));

                for out in output.iter_mut() {
                    *out *= gain;
                }
            }
        } else {
            router.param(
                &inputs.level,
                &channel.level,
                &mut self.buffers.gain_mod_input,
            );

            for (out, level) in output.iter_mut().zip(&self.buffers.gain_mod_input) {
                *out *= db_to_gain_fast(level.min(MAX_LEVEL_DB));
            }
        }

        if let Some(pan) = router.param_stationary(&inputs.pan, &channel.pan) {
            if pan.abs() >= 1e-6 {
                let gain = pan_gain(pan, channel_idx);

                for out in output.iter_mut() {
                    *out *= gain;
                }
            }
        } else {
            router.param(&inputs.pan, &channel.pan, &mut self.buffers.gain_mod_input);

            for (out, &pan) in output.iter_mut().zip(&self.buffers.gain_mod_input) {
                *out *= pan_gain(pan, channel_idx);
            }
        }

        if router.need_update_ui() {
            let level = self.out_volume_ballistics[target.channel_idx]
                .process(output, router.sample_rate());
            self.audio_end.update_out_volume(target.channel_idx, level);
        }
    }

    pub(crate) fn process(&mut self, ctx: &mut ProcessContext<L::EngineEnd>) {
        ctx.audio(self.id, self.output_slot)
            .for_voices(|rf, target, outputs| {
                self.process_voice(target, outputs, rf);
            })
            .for_channels(|rf, channel_idx| {
                self.channel_params[channel_idx]
                    .advance_smoothers(&rf.params().smooth_params, rf.params().samples);
            });
    }
}

impl<L: AmplifierLinks> SynthModule for Amplifier<L> {
    fn id(&self) -> ModuleId {
        self.id
    }

    fn inputs(&self) -> &'static [InputMeta] {
        static INPUTS: &[InputMeta] = &[
            InputMeta::direct_audio(Input::Audio),
            InputMeta::control(Input::Gain),
            InputMeta::control(Input::Level),
            InputMeta::control(Input::Pan),
        ];

        INPUTS
    }

    fn output_type(&self) -> DataType {
        DataType::Audio
    }

    fn output_slot(&self) -> usize {
        self.output_slot
    }

    fn set_output_slot(&mut self, slot: usize) {
        self.output_slot = slot;
    }

    fn set_input_slots(&mut self, inputs: &[InputSlots]) {
        self.inputs = Inputs::from_slots(inputs);
    }

    fn update_input_amount(&mut self, input_type: Input, src_slot: usize, amount: StereoSample) {
        self.inputs.update_amount(input_type, src_slot, amount);
    }

    fn process_ui_events(&mut self) {
        while let Some(event) = self.audio_end.pop_event() {
            match event {
                UiEvent::InputParam { input, value } => match input {
                    Input::Gain => self.set_gain(value),
                    Input::Level => self.set_level(value),
                    Input::Pan => self.set_pan(value),
                    _ => (),
                },
            }
        }
    }
}
