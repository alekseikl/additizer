use crate::synth_engine::{
    StereoSample,
    buffer::{Buffer, zero_buffer},
    routing::{DataType, Input, MAX_VOICES, ModuleId, ModuleType, NUM_CHANNELS, Router},
    smooth::SmoothedSample,
    synth_module::{
        ModInput, ModuleConfigBox, ProcessParams, SynthModule, VoiceRouter, VoiceRouterFactory,
    },
};
use itertools::izip;
use serde::{Deserialize, Serialize};

mod link;
mod ui_bridge;

use link::{AudioEnd, UiEnd, UiEvent, create_link_pair};
pub use ui_bridge::{UiBridge, UiState};

#[derive(Clone, Serialize, Deserialize)]
pub struct ChannelParams {
    gain: SmoothedSample,
}

impl Default for ChannelParams {
    fn default() -> Self {
        Self { gain: 0.0.into() }
    }
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct AmplifierConfig {
    label: Option<String>,
    channels: [ChannelParams; NUM_CHANNELS],
}

struct Voice {
    output: Buffer,
}

impl Voice {
    pub fn new() -> Self {
        Self {
            output: zero_buffer(),
        }
    }
}

impl Default for Voice {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Default)]
struct Channel {
    params: ChannelParams,
    voices: [Voice; MAX_VOICES],
}

struct Buffers {
    input: Buffer,
    gain_mod_input: Buffer,
}

pub struct Amplifier {
    id: ModuleId,
    label: String,
    config: ModuleConfigBox<AmplifierConfig>,
    buffers: Buffers,
    audio_end: AudioEnd,
    ui_end: Option<UiEnd>,
    channels: [Channel; NUM_CHANNELS],
}

impl Amplifier {
    pub fn new(id: ModuleId, config: ModuleConfigBox<AmplifierConfig>) -> Self {
        let (audio_end, ui_end) = create_link_pair();

        let mut amp = Self {
            id,
            label: format!("Amplifier {id}"),
            config,
            buffers: Buffers {
                input: zero_buffer(),
                gain_mod_input: zero_buffer(),
            },
            audio_end,
            ui_end: Some(ui_end),
            channels: Default::default(),
        };

        load_module_config_no_params!(amp);
        amp
    }

    pub fn take_ui_end(&mut self) -> Option<UiEnd> {
        self.ui_end.take()
    }

    pub fn return_ui_end(&mut self, ui_end: UiEnd) {
        assert!(self.ui_end.is_none(), "ui_end not taken");
        self.ui_end = Some(ui_end);
    }

    pub fn get_ui_state(&self) -> UiState {
        UiState {
            gain: get_smoothed_param!(self, gain),
        }
    }

    set_smoothed_param!(set_gain, gain);

    fn handle_ui_events(&mut self) {
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

    fn process_channel_voice(
        channel: &mut ChannelParams,
        voice: &mut Voice,
        buffers: &mut Buffers,
        router: &mut VoiceRouter<'_, '_>,
    ) {
        router.buff_param(Input::Gain, &mut channel.gain, &mut buffers.gain_mod_input);

        let input = router.buffer(Input::Audio, &mut buffers.input);

        for (out, input, modulation) in
            izip!(voice.output.iter_mut(), input, buffers.gain_mod_input).take(router.samples())
        {
            *out = input * modulation;
        }
    }
}

impl SynthModule for Amplifier {
    fn id(&self) -> ModuleId {
        self.id
    }

    fn label(&self) -> String {
        self.label.clone()
    }

    fn set_label(&mut self, label: String) {
        self.label = label.clone();
        self.config.lock().label = Some(label);
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::Amplifier
    }

    fn inputs(&self) -> &'static [ModInput] {
        static INPUTS: &[ModInput] = &[
            ModInput::buffer(Input::Audio),
            ModInput::buffer(Input::Gain),
        ];

        INPUTS
    }

    fn output(&self) -> DataType {
        DataType::Buffer
    }

    fn process(&mut self, process_params: &ProcessParams, router: &mut dyn Router) {
        self.handle_ui_events();

        let mut rf = VoiceRouterFactory::new(self.id, router, process_params);

        for (channel_idx, channel) in self.channels.iter_mut().enumerate() {
            for (seq_idx, voice_idx) in process_params.active_voices.iter().enumerate() {
                let voice = &mut channel.voices[*voice_idx];
                let mut voice_router = rf.for_voice(*voice_idx, channel_idx, seq_idx);

                Self::process_channel_voice(
                    &mut channel.params,
                    voice,
                    &mut self.buffers,
                    &mut voice_router,
                );
            }
        }
    }

    fn get_buffer_output(&self, voice_idx: usize, channel: usize) -> &Buffer {
        &self.channels[channel].voices[voice_idx].output
    }
}
