use std::array;

use itertools::izip;

mod config;
mod link;
mod ui_bridge;

pub use config::AmplifierConfig;
use link::{AudioEnd, UiEnd, UiEvent, create_link_pair};
pub use ui_bridge::AmplifierUiBridge;

use crate::synth_engine::{
    SmoothedSampleParams, StereoSample,
    buffer::{Buffer, VoicesLayout, zero_buffer},
    level_ballistics::LevelBallistics,
    routing::{
        AudioRouterType, DataType, Input, InputMeta, InputSlots, ModuleId, NUM_CHANNELS,
        ProcessContext, SamplesOutput, SpectralInputSlot, VoiceRouter,
    },
    smooth::SmoothedSample,
    synth_module::SynthModule,
};

struct ChannelParams {
    gain: SmoothedSample,
}

impl ChannelParams {
    fn from_config(c: &AmplifierConfig, channel_idx: usize) -> Self {
        Self {
            gain: c.gain[channel_idx].into(),
        }
    }

    pub fn advance_smoothers(&mut self, smooth_params: &SmoothedSampleParams, samples: usize) {
        self.gain.advance(smooth_params, samples);
    }
}

pub struct Inputs {
    audio: Option<usize>,
    gain: InputSlots,
}

impl Default for Inputs {
    fn default() -> Self {
        Self {
            audio: None,
            gain: InputSlots::empty(Input::Gain),
        }
    }
}

impl Inputs {
    fn from_slots(inputs: &[InputSlots], _spectral_inputs: &[SpectralInputSlot]) -> Self {
        let mut result = Self::default();

        for input in inputs {
            match input.input_type {
                Input::Audio => result.audio = input.slots.first().map(|s| s.src_slot),
                Input::Gain => result.gain = input.clone(),
                _ => (),
            }
        }

        result
    }

    fn update_amount(&mut self, input_type: Input, src_slot: usize, amount: StereoSample) {
        if input_type == Input::Gain {
            self.gain.update_amount(src_slot, amount);
        }
    }
}

type Router<'v, 'f, 'c> = VoiceRouter<'v, 'f, 'c, AudioRouterType>;

struct Buffers {
    gain_mod_input: Buffer,
}

pub struct Amplifier {
    id: ModuleId,
    channel_params: [ChannelParams; NUM_CHANNELS],
    buffers: Buffers,
    audio_end: AudioEnd,
    ui_end: Option<UiEnd>,
    inputs: Inputs,
    output_slot: usize,
    out_volume_ballistics: [LevelBallistics; NUM_CHANNELS],
}

impl Amplifier {
    pub fn new(id: ModuleId) -> Self {
        Self::from_config(&AmplifierConfig {
            id,
            ..AmplifierConfig::default()
        })
    }

    pub fn from_config(config: &config::AmplifierConfig) -> Self {
        let (audio_end, ui_end) = create_link_pair();

        Self {
            id: config.id,
            channel_params: array::from_fn(|channel_idx| {
                ChannelParams::from_config(config, channel_idx)
            }),
            buffers: Buffers {
                gain_mod_input: zero_buffer(),
            },
            audio_end,
            ui_end: Some(ui_end),
            inputs: Inputs::default(),
            output_slot: usize::MAX,
            out_volume_ballistics: [LevelBallistics::default(); NUM_CHANNELS],
        }
    }

    pub fn get_config(&self) -> AmplifierConfig {
        AmplifierConfig {
            id: self.id,
            gain: get_smoothed_param!(self, gain),
        }
    }

    set_smoothed_param!(set_gain, gain);

    fn process_voice(
        &mut self,
        output: &mut VoicesLayout<SamplesOutput>,
        mut router: Router<'_, '_, '_>,
    ) {
        let channel_idx = router.channel_idx();
        let voice_idx = router.voice_idx();
        let inputs = &self.inputs;
        let channel = &mut self.channel_params[channel_idx];
        let output = output[channel_idx][voice_idx].output(router.samples());

        router.buff_param(
            &inputs.gain,
            &mut channel.gain,
            &mut self.buffers.gain_mod_input,
        );

        let input = router.buff(inputs.audio);

        for (out, input, modulation) in
            izip!(output.iter_mut(), input, &self.buffers.gain_mod_input)
        {
            *out = input * modulation;
        }

        if router.need_update_ui() {
            let level =
                self.out_volume_ballistics[channel_idx].process(output, router.sample_rate());
            self.audio_end.update_out_volume(channel_idx, level);
        }
    }
}

impl SynthModule for Amplifier {
    fn id(&self) -> ModuleId {
        self.id
    }

    fn inputs(&self) -> &'static [InputMeta] {
        static INPUTS: &[InputMeta] = &[
            InputMeta::direct_audio(Input::Audio),
            InputMeta::control(Input::Gain),
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

    fn set_input_slots(&mut self, inputs: &[InputSlots], spectral_inputs: &[SpectralInputSlot]) {
        self.inputs = Inputs::from_slots(inputs, spectral_inputs);
    }

    fn update_input_amount(&mut self, input_type: Input, src_slot: usize, amount: StereoSample) {
        self.inputs.update_amount(input_type, src_slot, amount);
    }

    fn process_ui_events(&mut self) {
        while let Some(event) = self.audio_end.pop_event() {
            match event {
                UiEvent::InputParam { input, value } => {
                    if input == Input::Gain {
                        self.set_gain(value)
                    }
                }
            }
        }
    }

    fn process(&mut self, ctx: &mut ProcessContext) {
        ctx.for_audio(self.id, self.output_slot, |router, output| {
            let num_active_voices = router.params().active_voices.len();

            for channel_idx in 0..NUM_CHANNELS {
                for seq_idx in 0..num_active_voices {
                    let playing_voice = router.params().active_voices[seq_idx];

                    self.process_voice(
                        output,
                        router.for_voice(channel_idx, playing_voice, seq_idx),
                    );
                }

                self.channel_params[channel_idx]
                    .advance_smoothers(&router.params().smooth_params, router.params().samples);
            }
        });
    }
}
