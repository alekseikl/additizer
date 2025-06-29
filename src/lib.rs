pub mod editor;
pub mod envelope;
pub mod oscillator;
pub mod params;
pub mod phase;
pub mod stereo_sample;
pub mod utils;
pub mod voice;

use nih_plug::prelude::*;
use rand_pcg::Pcg32;
use std::f32::consts;
use std::sync::Arc;
use stereo_sample::StereoSample;
use utils::GlobalParamValues;
use voice::{Voice, VoiceId};

use crate::params::AdditizerParams;
use crate::phase::SINE_TABLE_BITS;

const VOLUME_POLY_MOD_ID: u32 = 0;

pub struct Additizer {
    params: Arc<AdditizerParams>,
    sample_rate: f32,
    voices: Vec<Voice>,
    random: Pcg32,
    sine_table: Option<Arc<Vec<f32>>>,
}

impl Default for Additizer {
    fn default() -> Self {
        Self {
            params: Arc::new(AdditizerParams::default()),
            sample_rate: 1.0,
            voices: Vec::new(),
            random: Pcg32::new(142, 997),
            sine_table: None,
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
    fn handle_note_on(&mut self, id: VoiceId, mut _terminate: impl FnMut(&VoiceId)) {
        if let Some(voice) = self.voices.iter_mut().find(|v| v.id().match_by_note(id)) {
            voice.fade_out();
        }

        self.voices.push(Voice::new(
            &mut self.random,
            id,
            self.params.unison.value() as usize,
            self.sine_table.as_ref().unwrap(),
        ));
    }

    fn handle_note_off(&mut self, id: VoiceId) {
        self.voices
            .iter_mut()
            .filter(|v| v.match_releasing(id, false))
            .for_each(|v| v.release());
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

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        editor::create(self.params.clone(), self.params.editor_state.clone())
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        self.sample_rate = buffer_config.sample_rate;

        let table_size = 1 << SINE_TABLE_BITS;
        let mut sine_table: Vec<f32> = vec![0.0; table_size];
        let step = 1.0 / table_size as f32;

        for (idx, value) in sine_table.iter_mut().enumerate() {
            *value = (step * idx as f32 * consts::TAU).sin();
        }

        self.sine_table = Some(Arc::new(sine_table));

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
        let harmonics = self.params.harmonics.lock().unwrap().clone();
        let subharmonics = self
            .params
            .subharmonics
            .lock()
            .unwrap()
            .clone()
            .into_iter()
            .rev()
            .collect();
        let tail_harmonics = self
            .params
            .tail_harmonics
            .load(std::sync::atomic::Ordering::Relaxed);

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

            let param_values = GlobalParamValues {
                volume: self.params.volume.smoothed.next(),
                harmonics: &harmonics,
                subharmonics: &subharmonics,
                tail_harmonics,
                detune: self.params.detune.smoothed.next(),
            };

            let mut result = StereoSample(0.0, 0.0);

            for voice in self.voices.iter_mut() {
                result += voice.tick(sample_rate, &param_values);

                if voice.is_done() {
                    // nih_log!("VoiceTerminated: {:?}", voice.id());
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

// impl Vst3Plugin for Additizer {
//     const VST3_CLASS_ID: [u8; 16] = *b"Additizer1111337";
//     const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] = &[
//         Vst3SubCategory::Instrument,
//         Vst3SubCategory::Synth,
//         Vst3SubCategory::Stereo,
//     ];
// }

nih_export_clap!(Additizer);
// nih_export_vst3!(Additizer);
