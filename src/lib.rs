#![allow(clippy::new_without_default)]

pub mod editor;
pub mod params;
pub mod phase;
pub mod synth_engine;
pub mod utils;

use crate::params::AdditizerParams;
use crate::synth_engine::buffer::BUFFER_SIZE;
use crate::synth_engine::{SynthEngine, VoiceId};
use nih_plug::prelude::*;
use std::sync::Arc;

pub struct Additizer {
    params: Arc<AdditizerParams>,
    synth_engine: SynthEngine,
}

impl Default for Additizer {
    fn default() -> Self {
        Self {
            params: Arc::new(AdditizerParams::default()),
            synth_engine: SynthEngine::new(),
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
        &mut self,
        context: &mut impl ProcessContext<Self>,
        event: NoteEvent<()>,
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
                let terminated = self.synth_engine.note_on(voice_id, channel, note, velocity);
                terminate_voice(terminated);
            }
            NoteEvent::NoteOff { note, .. } => {
                self.synth_engine.note_off(note);
            }
            NoteEvent::Choke { note, .. } => {
                let terminated = self.synth_engine.choke(note);
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
        editor::create(self.params.clone(), self.params.editor_state.clone())
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        self.synth_engine.init(buffer_config.sample_rate);
        true
    }

    fn reset(&mut self) {}

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let mut next_event = context.next_event();

        self.synth_engine.update_harmonics(
            self.params.harmonics.lock().as_deref().unwrap(),
            self.params
                .tail_harmonics
                .load(std::sync::atomic::Ordering::Relaxed),
        );

        for (block_idx, mut block) in buffer.iter_blocks(BUFFER_SIZE) {
            let sample_idx = block_idx * BUFFER_SIZE;
            let block_size = block.samples();

            while let Some(event) = next_event {
                if event.timing() > (sample_idx + block_size) as u32 {
                    break;
                }

                self.process_event(context, event, sample_idx as u32);
                next_event = context.next_event();
            }

            let terminated = self.synth_engine.process(block_size);

            for voice in terminated {
                context.send_event(voice.terminated_event(sample_idx as u32));
            }

            let output = self.synth_engine.get_output();
            let output = &output[..block_size];

            block.get_mut(0).unwrap().copy_from_slice(output);
            block.get_mut(1).unwrap().copy_from_slice(output);
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
