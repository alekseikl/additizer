use std::array;

use crate::{
    synth_engine::{MAX_LEVEL_DB, db_to_gain_fast},
    synth_engine::{
        SmoothedSampleParams, StereoSample,
        buffer::{Buffer, VoicesLayout, copy_or_add_to_buffer, zero_buffer},
        level_ballistics::LevelBallistics,
        routing::{
            AudioRouterType, DataType, Input, InputMeta, InputSlots, LevelType, MixedSlots,
            ModuleId, ProcessContext, RouterFactory, SamplesOutput, VoiceTarget,
        },
        smooth::SmoothedSample,
        synth_module::SynthModule,
        types::Smp,
        voices::NUM_CHANNELS,
    },
};

mod config;
mod link;
pub mod stub;

pub use config::{MAX_INPUTS, MixerConfig};
pub use link::{MixerAudioEnd, MixerLinks, MixerUiEnd, UiEvent, UiUpdate};

struct InputChannelParams {
    level: SmoothedSample,
    gain: SmoothedSample,
}

struct ChannelParams {
    input_params: [InputChannelParams; MAX_INPUTS as usize],
    output_level: SmoothedSample,
    output_gain: SmoothedSample,
}

impl ChannelParams {
    fn from_config(c: &config::MixerConfig, channel_idx: usize) -> Self {
        Self {
            input_params: c.inputs.map(|input| InputChannelParams {
                level: input.level[channel_idx].into(),
                gain: input.gain[channel_idx].into(),
            }),
            output_level: c.output_level[channel_idx].into(),
            output_gain: c.output_gain[channel_idx].into(),
        }
    }

    pub fn advance_smoothers(&mut self, smooth_params: &SmoothedSampleParams, samples: usize) {
        for input in &mut self.input_params {
            input.level.advance(smooth_params, samples);
            input.gain.advance(smooth_params, samples);
        }

        self.output_level.advance(smooth_params, samples);
        self.output_gain.advance(smooth_params, samples);
    }
}

struct InputParams {
    volume_type: LevelType,
}

struct Params {
    num_inputs: u8,
    inputs: [InputParams; MAX_INPUTS as usize],
    output_volume_type: LevelType,
}

impl Params {
    fn from_config(c: &config::MixerConfig) -> Self {
        Self {
            num_inputs: c.num_inputs.clamp(1, MAX_INPUTS),
            inputs: c.inputs.map(|input| InputParams {
                volume_type: input.volume_type,
            }),
            output_volume_type: c.output_volume_type,
        }
    }
}

pub struct Inputs {
    gain: MixedSlots,
    level: MixedSlots,
    audio_mix: [Option<usize>; MAX_INPUTS as usize],
    gain_mix: [MixedSlots; MAX_INPUTS as usize],
    level_mix: [MixedSlots; MAX_INPUTS as usize],
}

impl Default for Inputs {
    fn default() -> Self {
        Self {
            gain: MixedSlots::new(Input::LinLevel),
            level: MixedSlots::new(Input::Level),
            audio_mix: [None; MAX_INPUTS as usize],
            gain_mix: array::from_fn(|idx| MixedSlots::new(Input::LinLevelMix(idx as u8))),
            level_mix: array::from_fn(|idx| MixedSlots::new(Input::LevelMix(idx as u8))),
        }
    }
}

impl Inputs {
    fn from_slots(inputs: &[InputSlots]) -> Self {
        let mut result = Self::default();

        for input in inputs {
            match input {
                InputSlots::Direct { input_type, slot } => {
                    if let Input::AudioMix(idx) = input_type
                        && *idx < MAX_INPUTS
                    {
                        result.audio_mix[*idx as usize] = Some(*slot);
                    }
                }
                InputSlots::Mixed(input) => match input.input_type {
                    Input::LinLevel => result.gain = input.clone(),
                    Input::Level => result.level = input.clone(),
                    Input::LinLevelMix(idx) if idx < MAX_INPUTS => {
                        result.gain_mix[idx as usize] = input.clone();
                    }
                    Input::LevelMix(idx) if idx < MAX_INPUTS => {
                        result.level_mix[idx as usize] = input.clone();
                    }
                    _ => (),
                },
                InputSlots::Spectral { .. } => (),
            }
        }

        result
    }

    fn update_amount(&mut self, input_type: Input, src_slot: usize, amount: StereoSample) {
        match input_type {
            Input::LinLevel => self.gain.update_amount(src_slot, amount),
            Input::Level => self.level.update_amount(src_slot, amount),
            Input::LinLevelMix(idx) if idx < MAX_INPUTS => {
                self.gain_mix[idx as usize].update_amount(src_slot, amount);
            }
            Input::LevelMix(idx) if idx < MAX_INPUTS => {
                self.level_mix[idx as usize].update_amount(src_slot, amount);
            }
            _ => (),
        }
    }
}

struct Buffers {
    level_mod: Buffer,
}

impl Buffers {
    fn new(buffer_size: usize) -> Self {
        Self {
            level_mod: zero_buffer(buffer_size),
        }
    }
}

pub struct Mixer<L: MixerLinks = stub::Links> {
    id: ModuleId,
    params: Params,
    channel_params: [ChannelParams; NUM_CHANNELS],
    buffers: Buffers,
    audio_end: L::AudioEnd,
    ui_end: Option<L::UiEnd>,
    inputs: Inputs,
    output_slot: usize,
    inputs_meta: Vec<InputMeta>,
    out_volume_ballistics: [LevelBallistics; NUM_CHANNELS],
}

impl<L: MixerLinks> Mixer<L> {
    pub fn take_ui_end(&mut self) -> Option<L::UiEnd> {
        self.ui_end.take()
    }

    pub const MAX_INPUTS: u8 = MAX_INPUTS;

    pub fn new(id: ModuleId, buffer_size: usize) -> Self {
        Self::from_config(
            &MixerConfig {
                id,
                ..MixerConfig::default()
            },
            buffer_size,
        )
    }

    pub fn from_config(config: &config::MixerConfig, buffer_size: usize) -> Self {
        let (audio_end, ui_end) = L::create_link_pair();

        let mut result = Self {
            id: config.id,
            params: Params::from_config(config),
            channel_params: array::from_fn(|channel_idx| {
                ChannelParams::from_config(config, channel_idx)
            }),
            buffers: Buffers::new(buffer_size),
            audio_end,
            ui_end,
            inputs: Inputs::default(),
            output_slot: usize::MAX,
            inputs_meta: Vec::with_capacity(1 + 2 * MAX_INPUTS as usize),
            out_volume_ballistics: [LevelBallistics::default(); NUM_CHANNELS],
        };
        result.build_inputs_meta();
        result
    }

    pub fn get_config(&self) -> MixerConfig {
        MixerConfig {
            id: self.id,
            num_inputs: self.params.num_inputs,
            inputs: array::from_fn(|input_idx| config::InputConfig {
                volume_type: self.params.inputs[input_idx].volume_type,
                level: StereoSample::from_iter(
                    self.channel_params
                        .iter()
                        .map(|channel| channel.input_params[input_idx].level.get()),
                ),
                gain: StereoSample::from_iter(
                    self.channel_params
                        .iter()
                        .map(|channel| channel.input_params[input_idx].gain.get()),
                ),
            }),
            output_volume_type: self.params.output_volume_type,
            output_level: get_smoothed_param!(self, output_level),
            output_gain: get_smoothed_param!(self, output_gain),
        }
    }

    set_smoothed_param!(set_output_level, output_level);
    set_smoothed_param!(set_output_gain, output_gain);

    pub fn set_num_inputs(&mut self, num_inputs: u8) {
        self.params.num_inputs = num_inputs.clamp(1, MAX_INPUTS);
        self.build_inputs_meta();
    }

    pub fn set_volume_type(&mut self, input_idx: u8, volume_type: LevelType) {
        let input_idx = input_idx.clamp(0, MAX_INPUTS) as usize;
        self.params.inputs[input_idx].volume_type = volume_type;
        self.build_inputs_meta();
    }

    pub fn set_output_volume_type(&mut self, output_volume_type: LevelType) {
        self.params.output_volume_type = output_volume_type;
        self.build_inputs_meta();
    }

    pub fn set_input_level(&mut self, input_idx: u8, level: StereoSample) {
        let input_idx = input_idx.clamp(0, MAX_INPUTS) as usize;

        for (channel, level) in self.channel_params.iter_mut().zip(level.iter()) {
            channel.input_params[input_idx].level.set(*level);
        }
    }

    pub fn set_input_gain(&mut self, input_idx: u8, gain: StereoSample) {
        let input_idx = input_idx.clamp(0, MAX_INPUTS) as usize;

        for (channel, gain) in self.channel_params.iter_mut().zip(gain.iter()) {
            channel.input_params[input_idx].gain.set(*gain);
        }
    }

    fn build_inputs_meta(&mut self) {
        self.inputs_meta.clear();

        for input_idx in 0..self.params.num_inputs {
            self.inputs_meta
                .push(InputMeta::direct_audio(Input::AudioMix(input_idx)));

            match self.params.inputs[input_idx as usize].volume_type {
                LevelType::Db => self
                    .inputs_meta
                    .push(InputMeta::control(Input::LevelMix(input_idx))),
                LevelType::Linear => self
                    .inputs_meta
                    .push(InputMeta::control(Input::LinLevelMix(input_idx))),
            }
        }

        match self.params.output_volume_type {
            LevelType::Db => self.inputs_meta.push(InputMeta::control(Input::Level)),
            LevelType::Linear => self.inputs_meta.push(InputMeta::control(Input::LinLevel)),
        }
    }

    #[inline(always)]
    fn to_gain(dbs: Smp) -> Smp {
        db_to_gain_fast(dbs.min(MAX_LEVEL_DB))
    }

    #[inline(always)]
    fn mix_input(
        output: &mut [Smp],
        input: &[Smp],
        gain_mod: impl Iterator<Item = Smp>,
        input_idx: u8,
    ) {
        let input = input
            .iter()
            .zip(gain_mod)
            .map(|(sample, gain_mod)| sample * gain_mod);

        copy_or_add_to_buffer(input_idx == 0, output, input);
    }

    #[inline(always)]
    fn modulate_output(output: &mut [Smp], gain_mod: impl Iterator<Item = Smp>) {
        for (out, gain_mod) in output.iter_mut().zip(gain_mod) {
            *out *= gain_mod;
        }
    }

    fn process_voice(
        &mut self,
        target: &VoiceTarget,
        outputs: &mut VoicesLayout<SamplesOutput>,
        rf: &mut RouterFactory<AudioRouterType, L::EngineEnd>,
    ) {
        let (mut router, mut voice_output) = rf.for_voice(target, outputs);
        let inputs = &self.inputs;
        let channel = &mut self.channel_params[target.channel_idx];
        let output = voice_output.output();

        for input_idx in 0..self.params.num_inputs {
            let input_params = &self.params.inputs[input_idx as usize];
            let input_channel = &mut channel.input_params[input_idx as usize];

            match input_params.volume_type {
                LevelType::Db => {
                    router.param(
                        &inputs.level_mix[input_idx as usize],
                        &input_channel.level,
                        &mut self.buffers.level_mod,
                    );
                    let gain_mod = self
                        .buffers
                        .level_mod
                        .iter()
                        .map(|level| Self::to_gain(*level));

                    Self::mix_input(
                        output,
                        router.direct(inputs.audio_mix[input_idx as usize]),
                        gain_mod,
                        input_idx,
                    );
                }
                LevelType::Linear => {
                    router.param(
                        &inputs.gain_mix[input_idx as usize],
                        &input_channel.gain,
                        &mut self.buffers.level_mod,
                    );
                    let gain_mod = self.buffers.level_mod.iter().copied();

                    Self::mix_input(
                        output,
                        router.direct(inputs.audio_mix[input_idx as usize]),
                        gain_mod,
                        input_idx,
                    );
                }
            }
        }

        match self.params.output_volume_type {
            LevelType::Db => {
                router.param(
                    &inputs.level,
                    &channel.output_level,
                    &mut self.buffers.level_mod,
                );
                let gain_mod = self
                    .buffers
                    .level_mod
                    .iter()
                    .map(|level| Self::to_gain(*level));

                Self::modulate_output(output, gain_mod);
            }
            LevelType::Linear => {
                router.param(
                    &inputs.gain,
                    &channel.output_gain,
                    &mut self.buffers.level_mod,
                );
                let gain_mod = self.buffers.level_mod.iter().copied();

                Self::modulate_output(output, gain_mod);
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
            .advance(|rf| {
                for channel in &mut self.channel_params {
                    channel.advance_smoothers(&rf.params().smooth, rf.params().samples);
                }
            });
    }
}

impl<L: MixerLinks> SynthModule for Mixer<L> {
    fn id(&self) -> ModuleId {
        self.id
    }

    fn inputs(&self) -> &[InputMeta] {
        &self.inputs_meta
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
                    Input::LinLevel => self.set_output_gain(value),
                    Input::Level => self.set_output_level(value),
                    Input::LinLevelMix(idx) => self.set_input_gain(idx, value),
                    Input::LevelMix(idx) => self.set_input_level(idx, value),
                    _ => (),
                },
                UiEvent::NumInputs(num_inputs) => {
                    self.set_num_inputs(num_inputs);
                    self.audio_end.refresh_routing();
                }
                UiEvent::InputVolumeType {
                    input_idx,
                    volume_type,
                } => {
                    self.set_volume_type(input_idx, volume_type);
                    self.audio_end.refresh_routing();
                }
                UiEvent::OutputVolumeType(volume_type) => {
                    self.set_output_volume_type(volume_type);
                    self.audio_end.refresh_routing();
                }
            }
        }
    }
}
