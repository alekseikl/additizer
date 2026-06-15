use std::array;

use itertools::izip;

mod config;
mod link;
mod ui_bridge;

pub use config::SpectralBlendConfig;
use link::{AudioEnd, UiEnd, UiEvent, create_link_pair};
pub use ui_bridge::SpectralBlendUiBridge;

use crate::synth_engine::{
    StereoSample,
    buffer::{VoicesLayout, new_voices_layout},
    routing::{
        DataType, Input, InputSlots, ModuleId, NUM_CHANNELS, ProcessContext,
        SpectralInputSlot, SpectralOutput, SpectralRouterType, VoiceEvent, VoiceRouter,
    },
    synth_module::{ModInput, SynthModule},
    types::Sample,
};

struct ChannelParams {
    blend: Sample,
}

impl ChannelParams {
    fn from_config(c: &config::SpectralBlendConfig, channel_idx: usize) -> Self {
        Self {
            blend: c.blend[channel_idx],
        }
    }
}

#[derive(Default)]
struct VoiceState {
    triggered: bool,
}

pub struct Inputs {
    spectrum: Option<usize>,
    spectrum_to: Option<usize>,
    blend: InputSlots,
}

impl Default for Inputs {
    fn default() -> Self {
        Self {
            spectrum: None,
            spectrum_to: None,
            blend: InputSlots::empty(Input::Blend),
        }
    }
}

impl Inputs {
    fn from_slots(inputs: &[InputSlots], spectral_inputs: &[SpectralInputSlot]) -> Self {
        let mut result = Self::default();

        for input in inputs {
            if input.input_type == Input::Blend {
                result.blend = input.clone();
            }
        }

        for input in spectral_inputs {
            match input.input_type {
                Input::Spectrum => result.spectrum = Some(input.slot),
                Input::SpectrumTo => result.spectrum_to = Some(input.slot),
                _ => (),
            }
        }

        result
    }

    fn update_amount(&mut self, input_type: Input, src_slot: usize, amount: StereoSample) {
        if input_type == Input::Blend {
            self.blend.update_amount(src_slot, amount);
        }
    }
}

type Router<'v, 'f, 'c> = VoiceRouter<'v, 'f, 'c, SpectralRouterType>;

pub struct SpectralBlend {
    id: ModuleId,
    channel_params: [ChannelParams; NUM_CHANNELS],
    audio_end: AudioEnd,
    ui_end: Option<UiEnd>,
    inputs: Inputs,
    output_slot: usize,
    voices: VoicesLayout<VoiceState>,
}

impl SpectralBlend {
    pub fn new(id: ModuleId) -> Self {
        Self::from_config(&SpectralBlendConfig {
            id,
            ..SpectralBlendConfig::default()
        })
    }

    pub fn from_config(config: &config::SpectralBlendConfig) -> Self {
        let (audio_end, ui_end) = create_link_pair();

        Self {
            id: config.id,
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

    pub fn get_config(&self) -> SpectralBlendConfig {
        SpectralBlendConfig {
            id: self.id,
            blend: get_stereo_param!(self, blend),
        }
    }

    set_stereo_param!(set_blend, blend, blend.clamp(0.0, 1.0));

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

        let blend = router
            .scalar_param(&inputs.blend, channel.blend, voice.triggered)
            .clamp(0.0, 1.0);
        let spectrum_from = router.spectral(inputs.spectrum, voice.triggered);
        let spectrum_to = router.spectral(inputs.spectrum_to, voice.triggered);

        for (out, from, to) in izip!(voice_output, spectrum_from, spectrum_to) {
            *out = from + (to - from) * blend;
        }

        if voice.triggered {
            voice.triggered = false;

            self.process_voice(output, router);
        }
    }
}

impl SynthModule for SpectralBlend {
    fn id(&self) -> ModuleId {
        self.id
    }

    fn inputs(&self) -> &'static [ModInput] {
        static INPUTS: &[ModInput] = &[
            ModInput::spectral(Input::Spectrum),
            ModInput::spectral(Input::SpectrumTo),
            ModInput::control(Input::Blend),
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

    fn process_events(&mut self, events: &[VoiceEvent]) {
        for channel in self.voices.iter_mut() {
            for event in events {
                if let VoiceEvent::Trigger { voice_idx, .. } = event {
                    channel[*voice_idx].triggered = true;
                }
            }
        }
    }

    fn process_ui_events(&mut self) {
        while let Some(event) = self.audio_end.pop_event() {
            if let UiEvent::InputParam {
                input: Input::Blend,
                value,
            } = event
            {
                self.set_blend(value);
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
