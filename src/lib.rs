#![allow(clippy::new_without_default)]

use const_format::concatcp;

mod default_scheme;
mod editor;
mod engine_factory;
mod params;
mod preset;
mod presets;
pub mod synth_engine;
mod utils;

use crate::editor::create_editor;
use crate::engine_factory::{EngineFactory, EngineHandle};
use crate::params::AdditizerParams;
use crate::synth_engine::{Expression, ExternalParamsBlock, MAX_VOICES, Note, SynthEngine};
use crate::utils::log;
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

        let external_params = Arc::new(ExternalParamsBlock {
            float_params: std::array::from_fn(|i| params.float_params[i].param.clone()),
        });

        let factory = Arc::new(EngineFactory::new(params.volume.clone(), external_params));

        Self {
            params,
            engine: None,
            factory,
            terminated_notes: Vec::with_capacity(128),
        }
    }
}

impl Additizer {
    fn process_event(synth: &mut SynthEngine, event: NoteEvent<()>, block_start: usize) {
        log!("Event: {:?}", event);

        match event {
            NoteEvent::NoteOn {
                timing,
                voice_id,
                channel,
                note,
                velocity,
            } => {
                synth.handle_note_on(
                    Note {
                        channel,
                        note,
                        velocity,
                        host_id: voice_id,
                    },
                    timing as usize - block_start,
                );
            }
            NoteEvent::NoteOff {
                timing,
                voice_id,
                channel,
                note,
                velocity,
            } => {
                synth.handle_note_off(
                    Note {
                        channel,
                        note,
                        velocity,
                        host_id: voice_id,
                    },
                    timing as usize - block_start,
                );
            }
            NoteEvent::Choke {
                voice_id,
                channel,
                note,
                ..
            } => {
                synth.handle_choke(Note {
                    channel,
                    note,
                    velocity: 0.0,
                    host_id: voice_id,
                });
            }
            NoteEvent::PolyVolume {
                timing,
                voice_id,
                channel,
                note,
                gain,
            } => {
                synth.handle_note_expression(
                    Note {
                        channel,
                        note,
                        velocity: 0.0,
                        host_id: voice_id,
                    },
                    Expression::Gain,
                    timing as usize - block_start,
                    gain,
                );
            }
            NoteEvent::PolyPan {
                timing,
                voice_id,
                channel,
                note,
                pan,
            } => {
                synth.handle_note_expression(
                    Note {
                        channel,
                        note,
                        velocity: 0.0,
                        host_id: voice_id,
                    },
                    Expression::Pan,
                    timing as usize - block_start,
                    pan,
                );
            }
            NoteEvent::PolyTuning {
                timing,
                voice_id,
                channel,
                note,
                tuning,
            } => {
                synth.handle_note_expression(
                    Note {
                        channel,
                        note,
                        velocity: 0.0,
                        host_id: voice_id,
                    },
                    Expression::Pitch,
                    timing as usize - block_start,
                    tuning,
                );
            }
            NoteEvent::PolyBrightness {
                timing,
                voice_id,
                channel,
                note,
                brightness,
            } => {
                synth.handle_note_expression(
                    Note {
                        channel,
                        note,
                        velocity: 0.0,
                        host_id: voice_id,
                    },
                    Expression::Timbre,
                    timing as usize - block_start,
                    brightness,
                );
            }
            NoteEvent::PolyPressure {
                timing,
                voice_id,
                channel,
                note,
                pressure,
            } => {
                synth.handle_note_expression(
                    Note {
                        channel,
                        note,
                        velocity: 0.0,
                        host_id: voice_id,
                    },
                    Expression::Pressure,
                    timing as usize - block_start,
                    pressure,
                );
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

    const MIDI_INPUT: MidiConfig = MidiConfig::MidiCCs;
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

        let mut synth = self.engine.as_deref().unwrap().lock();

        assert_no_alloc::assert_no_alloc(|| {
            let block_size = synth.block_size();
            let update_ui = self.params.editor_state.is_open();

            let mut next_event = context.next_event();

            for (block_start, block) in buffer.iter_blocks(block_size) {
                let samples = block.samples();
                let sample_to = block_start + samples;

                while let Some(event) =
                    next_event.take_if(|event| (event.timing() as usize) < sample_to)
                {
                    Self::process_event(&mut synth, event, block_start);
                    next_event = context.next_event();
                }

                let mut channels = block.into_iter();
                let mut channel_outputs = [channels.next().unwrap(), channels.next().unwrap()];

                synth.process(
                    samples,
                    update_ui,
                    &mut self.terminated_notes,
                    &mut channel_outputs,
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
