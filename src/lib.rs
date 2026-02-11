#![allow(clippy::new_without_default)]

use const_format::concatcp;

mod default_scheme;
mod editor;
mod params;
mod presets;
mod synth_engine;
mod utils;

use crate::default_scheme::build_default_scheme;
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
    fn terminated_event(&self, timing: usize) -> NoteEvent<()> {
        NoteEvent::VoiceTerminated {
            timing: timing as u32,
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
        timing: usize,
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
                let terminated = synth.note_on(voice_id, channel, note, velocity);
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
    const NAME: &'static str = concatcp!("Additizer", env!("GIT_COMMIT_SUFFIX"));
    const VENDOR: &'static str = "Alexey Klyotzin";
    const URL: &'static str = "https://github.com/alekseikl/additizer";
    const EMAIL: &'static str = "svbs8000@gmail.com";

    const VERSION: &'static str = concatcp!(env!("CARGO_PKG_VERSION"), env!("GIT_COMMIT_SUFFIX"));

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

        if synth.is_empty() {
            build_default_scheme(&mut synth);
        }

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
            let total_samples = buffer.samples();
            let desired_block_size = synth.block_size();

            let mut process =
                |synth: &mut SynthEngine,
                 mut sample_idx: usize,
                 sample_idx_to: usize,
                 context: &mut dyn ProcessContext<Self>| {
                    while sample_idx < sample_idx_to {
                        let samples = desired_block_size.min(sample_idx_to - sample_idx);

                        synth.process(
                            samples,
                            buffer
                                .as_slice()
                                .iter_mut()
                                .map(|buff| &mut buff[sample_idx..sample_idx + samples]),
                            &mut |voice| context.send_event(voice.terminated_event(sample_idx)),
                        );

                        sample_idx += samples;
                    }
                };

            let mut next_event = context.next_event();
            let mut sample_idx = 0usize;

            while let Some(event) = next_event {
                let sample_idx_to = event.timing() as usize;

                if sample_idx_to > sample_idx {
                    process(&mut synth, sample_idx, sample_idx_to, context);
                    sample_idx = sample_idx_to;
                }

                Self::process_event(&mut synth, context, event, sample_idx);
                next_event = context.next_event();
            }

            process(&mut synth, sample_idx, total_samples, context);
        });

        ProcessStatus::KeepAlive
    }
}

impl ClapPlugin for Additizer {
    const CLAP_ID: &'static str = concatcp!("com.alekseikl.additizer", env!("GIT_COMMIT_SUFFIX"));
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
