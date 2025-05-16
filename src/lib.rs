pub mod oscillator;
pub mod phase;
pub mod stereo_sample;
pub mod utils;
pub mod voice;

use nih_plug::prelude::*;
use std::sync::Arc;
use stereo_sample::StereoSample;
use voice::{Voice, VoiceId, VoiceParamValues};

const VOLUME_POLY_MOD_ID: u32 = 0;

pub struct Additizer {
    params: Arc<AdditizerParams>,
    sample_rate: f32,
    voices: Vec<Voice>,
}

#[derive(Params)]
struct AdditizerParams {
    #[id = "volume"]
    pub volume: FloatParam,
}

impl Default for Additizer {
    fn default() -> Self {
        Self {
            params: Arc::new(AdditizerParams::default()),
            sample_rate: 1.0,
            voices: Vec::new(),
        }
    }
}

impl Default for AdditizerParams {
    fn default() -> Self {
        Self {
            volume: FloatParam::new(
                "Volume",
                0.0,
                FloatRange::SymmetricalSkewed {
                    min: util::MINUS_INFINITY_DB,
                    max: 6.0,
                    factor: FloatRange::skew_factor(-1.0),
                    center: 0.0,
                },
            )
            .with_poly_modulation_id(VOLUME_POLY_MOD_ID)
            .with_smoother(SmoothingStyle::Linear(3.0))
            .with_step_size(0.01)
            .with_unit(" dB"),
        }
    }
}

macro_rules! param_for_modulation_id {
    ($self:ident, $poly_modulation_id:expr) => {
        match $poly_modulation_id {
            VOLUME_POLY_MOD_ID => Some(&$self.params.volume),
            _ => None,
        }
    };
}

impl Additizer {
    fn handle_note_on(&mut self, id: VoiceId, mut terminate: impl FnMut(&VoiceId)) {
        if let Some(idx) = self.voices.iter().position(|v| v.id().match_by_note(id)) {
            let voice = &self.voices[idx];

            terminate(voice.id());
            self.voices.remove(idx);
        }

        self.voices.push(Voice::new(0.0, id));
    }

    fn handle_note_off(&mut self, id: VoiceId) {
        self.voices
            .iter_mut()
            .filter(|v| v.match_releasing(id, false))
            .for_each(|v| v.set_releasing());
    }

    fn handle_choke(&mut self, id: VoiceId, mut terminate: impl FnMut(&VoiceId)) {
        self.voices
            .iter()
            .filter(|v| v.id().match_voice(id))
            .for_each(|v| terminate(v.id()));

        self.voices.retain(|v| !v.id().match_voice(id));
    }

    fn handle_poly_modulation(
        &mut self,
        sample_rate: f32,
        voice_id: i32,
        poly_modulation_id: u32,
        normalized_offset: f32,
    ) {
        if let (Some(voice), Some(param)) = (
            self.voices
                .iter_mut()
                .find(|v| v.id().match_by_voice_id(voice_id)),
            param_for_modulation_id!(self, poly_modulation_id),
        ) {
            voice.apply_poly_modulation(sample_rate, poly_modulation_id, param, normalized_offset);
        }
    }

    fn handle_mono_automation(
        &mut self,
        sample_rate: f32,
        poly_modulation_id: u32,
        normalized_value: f32,
    ) {
        if let Some(param) = param_for_modulation_id!(self, poly_modulation_id) {
            for voice in self.voices.iter_mut() {
                voice.apply_mono_automation(
                    sample_rate,
                    poly_modulation_id,
                    param,
                    normalized_value,
                );
            }
        }
    }
}

impl Plugin for Additizer {
    const NAME: &'static str = "Additizer";
    const VENDOR: &'static str = "Spectral Blaze";
    const URL: &'static str = "https://youtu.be/dQw4w9WgXcQ";
    const EMAIL: &'static str = "info@example.com";

    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout {
        main_input_channels: None,
        main_output_channels: NonZeroU32::new(2),
        ..AudioIOLayout::const_default()
    }];

    const MIDI_INPUT: MidiConfig = MidiConfig::Basic;
    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        self.sample_rate = buffer_config.sample_rate;

        true
    }

    fn reset(&mut self) {}

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let sample_rate = context.transport().sample_rate;
        let mut next_event = context.next_event();

        for (sample_idx, channel_samples) in buffer.iter_samples().enumerate() {
            while let Some(event) = next_event {
                if event.timing() > sample_idx as u32 {
                    break;
                }

                let terminate = |id: &VoiceId| {
                    context.send_event(NoteEvent::VoiceTerminated {
                        timing: sample_idx as u32,
                        voice_id: id.voice_id,
                        channel: id.channel,
                        note: id.note,
                    })
                };

                match event {
                    NoteEvent::NoteOn {
                        channel,
                        note,
                        voice_id,
                        ..
                    } => {
                        self.handle_note_on(VoiceId::new(channel, note, voice_id), terminate);
                    }
                    NoteEvent::NoteOff {
                        channel,
                        note,
                        voice_id,
                        ..
                    } => {
                        self.handle_note_off(VoiceId::new(channel, note, voice_id));
                    }
                    NoteEvent::Choke {
                        timing: _,
                        voice_id,
                        channel,
                        note,
                    } => {
                        self.handle_choke(VoiceId::new(channel, note, voice_id), terminate);
                    }
                    NoteEvent::PolyModulation {
                        timing: _,
                        voice_id,
                        poly_modulation_id,
                        normalized_offset,
                    } => {
                        self.handle_poly_modulation(
                            sample_rate,
                            voice_id,
                            poly_modulation_id,
                            normalized_offset,
                        );
                    }
                    NoteEvent::MonoAutomation {
                        timing: _,
                        poly_modulation_id,
                        normalized_value,
                    } => {
                        self.handle_mono_automation(
                            sample_rate,
                            poly_modulation_id,
                            normalized_value,
                        );
                    }
                    _ => (),
                }

                next_event = context.next_event();
            }

            let param_values = VoiceParamValues {
                volume: self.params.volume.smoothed.next(),
            };

            let mut result = StereoSample(0.0, 0.0);

            for voice in self.voices.iter_mut() {
                result += voice.tick(sample_rate, &param_values);

                if voice.is_done() {
                    nih_log!("VoiceTerminated: {:?}", voice.id());
                    context.send_event(NoteEvent::VoiceTerminated {
                        timing: sample_idx as u32,
                        voice_id: voice.id().voice_id,
                        channel: voice.id().channel,
                        note: voice.id().note,
                    });
                }
            }

            self.voices.retain(|v| !v.is_done());

            for (out_sample, result_sample) in channel_samples.into_iter().zip(result.iter()) {
                *out_sample = result_sample;
            }
        }

        ProcessStatus::KeepAlive
    }
}

impl ClapPlugin for Additizer {
    const CLAP_ID: &'static str = "com.spectral-blaze.additizer";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("Additive synthesizer");
    const CLAP_MANUAL_URL: Option<&'static str> = Some(Self::URL);
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::Instrument,
        ClapFeature::Synthesizer,
        ClapFeature::Stereo,
        ClapFeature::Mono,
        ClapFeature::Utility,
    ];
}

nih_export_clap!(Additizer);
