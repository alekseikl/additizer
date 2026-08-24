#![allow(clippy::new_without_default)]

use const_format::concatcp;

mod default_scheme;
mod editor;
mod engine_factory;
mod host_events;
mod params;
mod preset;
mod presets;
pub mod synth_engine;
mod utils;

use crate::editor::create_editor;
use crate::engine_factory::{EngineFactory, EngineHandle};
use crate::params::AdditizerParams;
use crate::synth_engine::{MAX_VOICES, Note};
// use crate::utils::log;
pub use egui;
use nice_plug::prelude::*;
use std::sync::Arc;

pub struct Additizer {
    params: Arc<AdditizerParams>,
    engine: Option<EngineHandle>,
    factory: Arc<EngineFactory>,
    terminated_notes: Vec<Note>,
}

impl Default for Additizer {
    fn default() -> Self {
        let params = Arc::new(AdditizerParams::default());
        let factory = Arc::new(EngineFactory::new());

        Self {
            params,
            engine: None,
            factory,
            terminated_notes: Vec::with_capacity(128),
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

    const MIDI_INPUT: MidiConfig = MidiConfig::MidiCCs;

    // Don't split a buffer
    const SAMPLE_ACCURATE_AUTOMATION: bool = false;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        create_editor(Arc::clone(&self.params.editor_state), self.factory.clone())
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        self.factory.set_host_sample_rate(buffer_config.sample_rate);
        self.params.config.set_factory(self.factory.clone());
        self.engine = Some(self.factory.get_engine());

        true
    }

    fn reset(&mut self) {
        self.engine.as_deref().unwrap().lock().reset();
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        if self
            .engine
            .as_ref()
            .is_none_or(|engine| self.factory.engine_changed(engine))
        {
            self.engine = Some(self.factory.get_engine());
        }

        // Mutex is contended only when routing is changed from the UI.
        let mut synth = self.engine.as_deref().unwrap().lock();

        assert_no_alloc::assert_no_alloc(|| {
            let block_size = synth.block_size();
            let update_ui = self.params.editor_state.is_open();
            let mut next_event = context.next_event();

            synth.set_automation_values(&self.params.ext_params);

            for (block_start, block) in buffer.iter_blocks(block_size) {
                let samples = block.samples();
                let sample_to = block_start + samples;

                while let Some(event) = next_event.take_if(|e| (e.timing() as usize) < sample_to) {
                    host_events::process_event(
                        &mut synth,
                        event,
                        block_start,
                        &self.params.ext_params,
                    );
                    next_event = context.next_event();
                }

                let mut channels = block.into_iter();

                synth.process(
                    samples,
                    update_ui,
                    &mut self.terminated_notes,
                    [channels.next().unwrap(), channels.next().unwrap()],
                );

                for note in self.terminated_notes.drain(..) {
                    context.send_event(NoteEvent::VoiceTerminated {
                        timing: sample_to as u32,
                        voice_id: note.host_id,
                        channel: note.channel,
                        note: note.note,
                    });
                }
            }
        });

        ProcessStatus::KeepAlive
    }
}

impl ClapPlugin for Additizer {
    const CLAP_ID: &'static str = concatcp!("com.alekseikl.additizer", env!("GIT_COMMIT_SUFFIX"));
    const CLAP_DESCRIPTION: Option<&'static str> = Some("Modular synthesizer");
    const CLAP_MANUAL_URL: Option<&'static str> = Some(Self::URL);
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::Instrument,
        ClapFeature::Synthesizer,
        ClapFeature::Stereo,
    ];

    const CLAP_POLY_MODULATION_CONFIG: Option<PolyModulationConfig> = Some(PolyModulationConfig {
        max_voice_capacity: MAX_VOICES as u32,
        supports_overlapping_voices: false,
    });
}

// impl Vst3Plugin for Additizer {
//     const VST3_CLASS_ID: [u8; 16] = *b"Additizer1111337";
//     const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] = &[
//         Vst3SubCategory::Instrument,
//         Vst3SubCategory::Synth,
//         Vst3SubCategory::Stereo,
//     ];
// }

nice_export_clap!(Additizer);
// nice_export_vst3!(Additizer);
