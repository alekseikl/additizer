#![allow(clippy::new_without_default)]

pub mod editor;
pub mod params;
pub mod synth_engine;
pub mod utils;

use crate::editor::create_editor;
use crate::params::AdditizerParams;
use crate::synth_engine::{ExternalParamsBlock, SynthEngine, VoiceId};
pub use egui_baseview::egui;
use nih_plug::prelude::*;
use parking_lot::Mutex;
use std::sync::Arc;

pub struct Additizer {
    params: Arc<AdditizerParams>,
    synth_engine: Arc<Mutex<SynthEngine>>,
}

impl Default for Additizer {
    fn default() -> Self {
        Self {
            params: Arc::new(AdditizerParams::default()),
            synth_engine: Arc::new(Mutex::new(SynthEngine::new())),
        }
    }
}

impl VoiceId {
    fn terminated_event(&self, timing: u32) -> NoteEvent<()> {
        NoteEvent::VoiceTerminated {
            timing,
            voice_id: self.voice_id,
            channel: self.channel,
            note: self.note,
        }
    }
}

impl Additizer {
    fn process_event(
        synth: &mut SynthEngine,
        context: &mut impl ProcessContext<Self>,
        event: NoteEvent<()>,
        samples: usize,
        timing: u32,
    ) {
        let mut terminate_voice = |voice: Option<VoiceId>| {
            if let Some(voice) = voice {
                context.send_event(voice.terminated_event(timing))
            }
        };

        match event {
            NoteEvent::NoteOn {
                channel,
                note,
                voice_id,
                velocity,
                ..
            } => {
                let terminated = synth.note_on(samples, voice_id, channel, note, velocity);
                terminate_voice(terminated);
            }
            NoteEvent::NoteOff { note, .. } => {
                synth.note_off(note);
            }
            NoteEvent::Choke { note, .. } => {
                let terminated = synth.choke(note);
                terminate_voice(terminated);
            }
            _ => (),
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
        create_editor(
            Arc::clone(&self.params.editor_state),
            Arc::clone(&self.synth_engine),
        )
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        let mut synth = self.synth_engine.lock();

        let external_params = ExternalParamsBlock {
            float_params: [
                Arc::clone(&self.params.float_param_1),
                Arc::clone(&self.params.float_param_2),
                Arc::clone(&self.params.float_param_3),
                Arc::clone(&self.params.float_param_4),
            ],
        };

        synth.init(
            Arc::clone(&self.params.config),
            Arc::clone(&self.params.volume),
            external_params,
            buffer_config.sample_rate,
        );

        true
    }

    fn reset(&mut self) {}

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let mut synth = self.synth_engine.lock();

        assert_no_alloc::assert_no_alloc(|| {
            let buffer_size = synth.get_buffer_size();
            let mut next_event = context.next_event();

            for (block_idx, mut block) in buffer.iter_blocks(buffer_size) {
                let sample_idx = block_idx * buffer_size;
                let block_size = block.samples();

                while let Some(event) = next_event {
                    if event.timing() > (sample_idx + block_size) as u32 {
                        break;
                    }

                    Self::process_event(&mut synth, context, event, block_size, sample_idx as u32);
                    next_event = context.next_event();
                }

                synth.process(block_size, block.iter_mut(), |voice: VoiceId| {
                    context.send_event(voice.terminated_event(sample_idx as u32))
                });
            }
        });

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
