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
    buffer::VoicesLayout,
    routing::{
        DataType, Input, InputMeta, InputSlots, ModuleId, NUM_CHANNELS, ProcessContext,
        RouterFactory, SpectralInputSlot, SpectralOutput, SpectralRouterType, VoiceTarget,
    },
    synth_module::SynthModule,
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
            blend: InputSlots::new(Input::Blend),
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

pub struct SpectralBlend {
    id: ModuleId,
    channel_params: [ChannelParams; NUM_CHANNELS],
    audio_end: AudioEnd,
    ui_end: Option<UiEnd>,
    inputs: Inputs,
    output_slot: usize,
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
            output_slot: usize::MAX,
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
        target: &VoiceTarget,
        outputs: &mut VoicesLayout<SpectralOutput>,
        rf: &mut RouterFactory<SpectralRouterType>,
    ) {
        let (mut router, mut voice_output) = rf.for_voice(target, outputs);
        let inputs = &self.inputs;
        let channel = &self.channel_params[target.channel_idx];

        let blend = router.scalar(&inputs.blend, channel.blend).clamp(0.0, 1.0);
        let spectrum_from = router.spectral(inputs.spectrum);
        let spectrum_to = router.spectral(inputs.spectrum_to);

        for (out, from, to) in izip!(voice_output.output(), spectrum_from, spectrum_to) {
            *out = from + (to - from) * blend;
        }

        if router.need_update_ui_mono() {
            self.audio_end.update_spectrum(voice_output.output());
        }
    }
}

impl SynthModule for SpectralBlend {
    fn id(&self) -> ModuleId {
        self.id
    }

    fn inputs(&self) -> &'static [InputMeta] {
        static INPUTS: &[InputMeta] = &[
            InputMeta::spectral(Input::Spectrum),
            InputMeta::spectral(Input::SpectrumTo),
            InputMeta::control(Input::Blend),
        ];

        INPUTS
    }

    fn output_type(&self) -> DataType {
        DataType::Spectral
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
        ctx.for_spectral(self.id, self.output_slot, |rf, target, outputs| {
            self.process_voice(target, outputs, rf);
        });
    }
}
