use core::f32;
use std::array;

use nih_plug::util::db_to_gain_fast;

mod config;
mod link;
mod ui_bridge;

pub use config::{MAX_INPUTS, SpectralMixerConfig};
use link::{AudioEnd, UiEnd, UiEvent, create_link_pair};
pub use ui_bridge::SpectralMixerUiBridge;

use crate::synth_engine::{
    StereoSample,
    buffer::{VoicesLayout, new_voices_layout},
    routing::{
        DataType, Input, InputSlots, MixType, ModuleId, ModuleType, NUM_CHANNELS, ProcessContext,
        SpectralInputSlot, SpectralOutput, SpectralRouterType, VoiceEvent, VoiceRouter, VolumeType,
    },
    synth_module::{ModInput, SynthModule},
    types::{ComplexSample, Sample},
};

const MAX_VOLUME: Sample = 24.0; // dB

struct InputChannelParams {
    level: Sample,
    gain: Sample,
}

struct ChannelParams {
    input_params: [InputChannelParams; MAX_INPUTS as usize],
    output_level: Sample,
    output_gain: Sample,
}

impl ChannelParams {
    fn from_config(c: &config::SpectralMixerConfig, channel_idx: usize) -> Self {
        Self {
            input_params: c.inputs.map(|input| InputChannelParams {
                level: input.level[channel_idx],
                gain: input.gain[channel_idx],
            }),
            output_level: c.output_level[channel_idx],
            output_gain: c.output_gain[channel_idx],
        }
    }
}

struct InputParams {
    mix_type: MixType,
    volume_type: VolumeType,
}

struct Params {
    num_inputs: u8,
    inputs: [InputParams; MAX_INPUTS as usize],
    output_volume_type: VolumeType,
}

impl Params {
    fn from_config(c: &config::SpectralMixerConfig) -> Self {
        Self {
            num_inputs: c.num_inputs,
            inputs: c.inputs.map(|input| InputParams {
                mix_type: input.mix_type,
                volume_type: input.volume_type,
            }),
            output_volume_type: c.output_volume_type,
        }
    }
}

#[derive(Default)]
struct VoiceState {
    triggered: bool,
}

pub struct Inputs {
    gain: InputSlots,
    level: InputSlots,
    spectrum_mix: [Option<usize>; MAX_INPUTS as usize],
    gain_mix: [InputSlots; MAX_INPUTS as usize],
    level_mix: [InputSlots; MAX_INPUTS as usize],
}

impl Default for Inputs {
    fn default() -> Self {
        Self {
            gain: InputSlots::empty(Input::Gain),
            level: InputSlots::empty(Input::Level),
            spectrum_mix: [None; MAX_INPUTS as usize],
            gain_mix: array::from_fn(|idx| InputSlots::empty(Input::GainMix(idx as u8))),
            level_mix: array::from_fn(|idx| InputSlots::empty(Input::LevelMix(idx as u8))),
        }
    }
}

impl Inputs {
    fn from_slots(inputs: &[InputSlots], spectral_inputs: &[SpectralInputSlot]) -> Self {
        let mut result = Self::default();

        for input in inputs {
            match input.input_type {
                Input::Gain => result.gain = input.clone(),
                Input::Level => result.level = input.clone(),
                Input::GainMix(idx) if idx < MAX_INPUTS => {
                    result.gain_mix[idx as usize] = input.clone();
                }
                Input::LevelMix(idx) if idx < MAX_INPUTS => {
                    result.level_mix[idx as usize] = input.clone();
                }
                _ => (),
            }
        }

        for input in spectral_inputs {
            if let Input::SpectrumMix(idx) = input.input_type
                && idx < MAX_INPUTS
            {
                result.spectrum_mix[idx as usize] = Some(input.slot);
            }
        }

        result
    }

    fn update_amount(&mut self, input_type: Input, src_slot: usize, amount: StereoSample) {
        match input_type {
            Input::Gain => self.gain.update_amount(src_slot, amount),
            Input::Level => self.level.update_amount(src_slot, amount),
            Input::GainMix(idx) if idx < MAX_INPUTS => {
                self.gain_mix[idx as usize].update_amount(src_slot, amount);
            }
            Input::LevelMix(idx) if idx < MAX_INPUTS => {
                self.level_mix[idx as usize].update_amount(src_slot, amount);
            }
            _ => (),
        }
    }
}

type Router<'v, 'f, 'c> = VoiceRouter<'v, 'f, 'c, SpectralRouterType>;

pub struct SpectralMixer {
    id: ModuleId,
    params: Params,
    channel_params: [ChannelParams; NUM_CHANNELS],
    audio_end: AudioEnd,
    ui_end: Option<UiEnd>,
    inputs: Inputs,
    output_slot: usize,
    voices: VoicesLayout<VoiceState>,
}

impl SpectralMixer {
    pub const MAX_INPUTS: u8 = MAX_INPUTS;

    pub fn new(id: ModuleId) -> Self {
        Self::from_config(&SpectralMixerConfig {
            id,
            ..SpectralMixerConfig::default()
        })
    }

    pub fn from_config(config: &config::SpectralMixerConfig) -> Self {
        let (audio_end, ui_end) = create_link_pair();

        Self {
            id: config.id,
            params: Params::from_config(config),
            channel_params: array::from_fn(|channel_idx| {
                ChannelParams::from_config(config, channel_idx)
            }),
            audio_end,
            ui_end: Some(ui_end),
            inputs: Inputs::default(),
            output_slot: 0,
            voices: new_voices_layout(),
        }
    }

    pub fn get_config(&self) -> SpectralMixerConfig {
        SpectralMixerConfig {
            id: self.id,
            num_inputs: self.params.num_inputs,
            inputs: array::from_fn(|input_idx| config::InputConfig {
                mix_type: self.params.inputs[input_idx].mix_type,
                volume_type: self.params.inputs[input_idx].volume_type,
                level: StereoSample::from_iter(
                    self.channel_params
                        .iter()
                        .map(|channel| channel.input_params[input_idx].level),
                ),
                gain: StereoSample::from_iter(
                    self.channel_params
                        .iter()
                        .map(|channel| channel.input_params[input_idx].gain),
                ),
            }),
            output_volume_type: self.params.output_volume_type,
            output_level: get_stereo_param!(self, output_level),
            output_gain: get_stereo_param!(self, output_gain),
        }
    }

    set_mono_param!(
        set_num_inputs,
        num_inputs,
        u8,
        num_inputs.clamp(1, MAX_INPUTS)
    );

    set_mono_param!(set_output_volume_type, output_volume_type, VolumeType);

    set_stereo_param!(set_output_level, output_level);
    set_stereo_param!(set_output_gain, output_gain);

    pub fn set_mix_type(&mut self, input_idx: u8, mix_type: MixType) {
        let input_idx = input_idx.clamp(0, MAX_INPUTS) as usize;
        self.params.inputs[input_idx].mix_type = mix_type;
    }

    pub fn set_volume_type(&mut self, input_idx: u8, volume_type: VolumeType) {
        let input_idx = input_idx.clamp(0, MAX_INPUTS) as usize;
        self.params.inputs[input_idx].volume_type = volume_type;
    }

    pub fn set_input_level(&mut self, input_idx: u8, level: StereoSample) {
        let input_idx = input_idx.clamp(0, MAX_INPUTS) as usize;

        for (channel, level) in self.channel_params.iter_mut().zip(level.iter()) {
            channel.input_params[input_idx].level = *level;
        }
    }

    pub fn set_input_gain(&mut self, input_idx: u8, gain: StereoSample) {
        let input_idx = input_idx.clamp(0, MAX_INPUTS) as usize;

        for (channel, gain) in self.channel_params.iter_mut().zip(gain.iter()) {
            channel.input_params[input_idx].gain = *gain;
        }
    }

    #[inline(always)]
    fn to_gain(vol: Sample) -> Sample {
        db_to_gain_fast(vol.min(MAX_VOLUME))
    }

    fn process_voice(
        &mut self,
        output: &mut VoicesLayout<SpectralOutput>,
        mut router: Router<'_, '_, '_>,
    ) {
        let channel_idx = router.channel_idx();
        let voice_idx = router.voice_idx();
        let inputs = &self.inputs;
        let channel = &self.channel_params[channel_idx];
        let voice = &mut self.voices[channel_idx][voice_idx];
        let voice_output = output[channel_idx][voice_idx].advance();

        voice_output.fill(ComplexSample::ZERO);

        for input_idx in 0..self.params.num_inputs {
            let input_params = &self.params.inputs[input_idx as usize];
            let input_channel = &channel.input_params[input_idx as usize];

            let gain = match input_params.volume_type {
                VolumeType::Db => Self::to_gain(router.scalar_param(
                    &inputs.level_mix[input_idx as usize],
                    input_channel.level,
                    voice.triggered,
                )),
                VolumeType::Gain => router.scalar_param(
                    &inputs.gain_mix[input_idx as usize],
                    input_channel.gain,
                    voice.triggered,
                ),
            };

            let spectrum =
                router.spectral(inputs.spectrum_mix[input_idx as usize], voice.triggered);

            let iter = voice_output
                .iter_mut()
                .zip(spectrum.map(|input| input * gain));

            if input_idx == 0 {
                iter.for_each(|(out, input)| *out = input);
            } else {
                match input_params.mix_type {
                    MixType::Add => {
                        iter.for_each(|(out, input)| *out += input);
                    }
                    MixType::Subtract => {
                        iter.for_each(|(out, input)| *out -= input);
                    }
                    MixType::Multiply => {
                        iter.enumerate().for_each(|(idx, (out, input))| {
                            *out *= input * idx as Sample * f32::consts::PI
                        });
                    }
                }
            }
        }

        let output_gain = match self.params.output_volume_type {
            VolumeType::Db => Self::to_gain(router.scalar_param(
                &inputs.level,
                channel.output_level,
                voice.triggered,
            )),
            VolumeType::Gain => {
                router.scalar_param(&inputs.gain, channel.output_gain, voice.triggered)
            }
        };

        for out in voice_output.iter_mut() {
            *out *= output_gain;
        }

        if voice.triggered {
            voice.triggered = false;

            self.process_voice(output, router);
        }
    }
}

impl SynthModule for SpectralMixer {
    fn id(&self) -> ModuleId {
        self.id
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::SpectralMixer
    }

    fn inputs(&self) -> &'static [ModInput] {
        static INPUTS: &[ModInput] = &[
            ModInput::control(Input::Gain),
            ModInput::control(Input::Level),
            ModInput::spectral(Input::SpectrumMix(0)),
            ModInput::control(Input::GainMix(0)),
            ModInput::control(Input::LevelMix(0)),
            ModInput::spectral(Input::SpectrumMix(1)),
            ModInput::control(Input::GainMix(1)),
            ModInput::control(Input::LevelMix(1)),
            ModInput::spectral(Input::SpectrumMix(2)),
            ModInput::control(Input::GainMix(2)),
            ModInput::control(Input::LevelMix(2)),
            ModInput::spectral(Input::SpectrumMix(3)),
            ModInput::control(Input::GainMix(3)),
            ModInput::control(Input::LevelMix(3)),
            ModInput::spectral(Input::SpectrumMix(4)),
            ModInput::control(Input::GainMix(4)),
            ModInput::control(Input::LevelMix(4)),
            ModInput::spectral(Input::SpectrumMix(5)),
            ModInput::control(Input::GainMix(5)),
            ModInput::control(Input::LevelMix(5)),
        ];

        INPUTS
    }

    fn output(&self) -> DataType {
        DataType::Spectral
    }

    fn output_slot(&self) -> usize {
        self.output_slot
    }

    fn set_slots(
        &mut self,
        inputs: &[InputSlots],
        spectral_inputs: &[SpectralInputSlot],
        output_slot: usize,
    ) {
        self.inputs = Inputs::from_slots(inputs, spectral_inputs);
        self.output_slot = output_slot;
    }

    fn update_input_amount(&mut self, input_type: Input, src_slot: usize, amount: StereoSample) {
        self.inputs.update_amount(input_type, src_slot, amount);
    }

    fn handle_events(&mut self, events: &[VoiceEvent]) {
        for channel in self.voices.iter_mut() {
            for event in events {
                if let VoiceEvent::Trigger { voice_idx, .. } = event {
                    channel[*voice_idx].triggered = true;
                }
            }
        }
    }

    fn handle_ui_events(&mut self) {
        while let Some(event) = self.audio_end.pop_event() {
            match event {
                UiEvent::InputParam { input, value } => match input {
                    Input::Gain => self.set_output_gain(value),
                    Input::Level => self.set_output_level(value),
                    Input::GainMix(idx) => self.set_input_gain(idx, value),
                    Input::LevelMix(idx) => self.set_input_level(idx, value),
                    _ => (),
                },
                UiEvent::NumInputs(num_inputs) => self.set_num_inputs(num_inputs),
                UiEvent::MixType {
                    input_idx,
                    mix_type,
                } => self.set_mix_type(input_idx, mix_type),
                UiEvent::VolumeType {
                    input_idx,
                    volume_type,
                } => self.set_volume_type(input_idx, volume_type),
                UiEvent::OutputVolumeType(volume_type) => self.set_output_volume_type(volume_type),
            }
        }
    }

    fn process(&mut self, ctx: &mut ProcessContext) {
        ctx.for_spectral(self.id, self.output_slot, |router, output| {
            let num_active_voices = router.params().active_voices.len();
            let spectrum_channels = router.params().spectrum_channels;

            for channel_idx in 0..spectrum_channels {
                for seq_idx in 0..num_active_voices {
                    let voice_idx = router.params().active_voices[seq_idx];

                    self.process_voice(output, router.for_voice(channel_idx, voice_idx, seq_idx));
                }
            }
        });
    }
}
