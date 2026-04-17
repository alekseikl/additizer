#![allow(clippy::new_without_default)]

use const_format::concatcp;
use smallvec::SmallVec;

mod default_scheme;
mod editor;
mod params;
mod presets;
mod synth_engine;
mod utils;

use crate::default_scheme::build_default_scheme;
use crate::editor::create_editor;
use crate::params::AdditizerParams;
use crate::synth_engine::{Expression, ExternalParamsBlock, SynthEngine};
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

trait EventUtils {
    fn is_barrier(&self) -> bool;
}

impl EventUtils for NoteEvent<()> {
    fn is_barrier(&self) -> bool {
        matches!(self, NoteEvent::NoteOn { .. } | NoteEvent::NoteOff { .. })
    }
}

struct EventReorderer<'a, C: ProcessContext<Additizer>> {
    context: &'a mut C,
    buffer: SmallVec<[NoteEvent<()>; 32]>,
    stashed: Option<NoteEvent<()>>,
}

impl<'a, C: ProcessContext<Additizer>> EventReorderer<'a, C> {
    fn new(context: &'a mut C) -> Self {
        Self {
            context,
            buffer: SmallVec::new(),
            stashed: None,
        }
    }

    fn priority(event: &NoteEvent<()>) -> u8 {
        match event {
            NoteEvent::Choke { .. } => 3,
            NoteEvent::NoteOff { .. } => 2,
            NoteEvent::NoteOn { .. } => 1,
            _ => 0,
        }
    }

    fn next_event(&mut self) -> Option<NoteEvent<()>> {
        if !self.buffer.is_empty() {
            return self.buffer.pop();
        }

        let first = self.stashed.take().or_else(|| self.context.next_event())?;
        let current_timing = first.timing();

        self.buffer.push(first);

        while let Some(event) = self.context.next_event() {
            if event.timing() == current_timing {
                self.buffer.push(event);
            } else {
                self.stashed.replace(event);
                break;
            }
        }

        self.buffer.sort_by_key(Self::priority);
        self.buffer.pop()
    }
}

impl Additizer {
    fn process_event(synth: &mut SynthEngine, event: NoteEvent<()>) {
        // nih_log!("Event: {:?}", event);

        match event {
            NoteEvent::NoteOn {
                channel,
                note,
                velocity,
                ..
            } => {
                synth.handle_note_on(channel, note, velocity);
            }
            NoteEvent::NoteOff {
                channel,
                note,
                velocity,
                ..
            } => {
                synth.handle_note_off(channel, note, velocity);
            }
            NoteEvent::Choke { channel, note, .. } => {
                synth.handle_choke(channel, note);
            }
            NoteEvent::PolyVolume {
                channel,
                note,
                gain,
                ..
            } => {
                synth.handle_note_expression(channel, note, Expression::Gain, gain);
            }
            NoteEvent::PolyPan {
                channel, note, pan, ..
            } => {
                synth.handle_note_expression(channel, note, Expression::Pan, pan);
            }
            NoteEvent::PolyTuning {
                channel,
                note,
                tuning,
                ..
            } => {
                synth.handle_note_expression(channel, note, Expression::Pitch, tuning);
            }
            NoteEvent::PolyBrightness {
                channel,
                note,
                brightness,
                ..
            } => {
                synth.handle_note_expression(channel, note, Expression::Timbre, brightness);
            }
            NoteEvent::PolyPressure {
                channel,
                note,
                pressure,
                ..
            } => {
                synth.handle_note_expression(channel, note, Expression::Pressure, pressure);
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
        struct BlocksHandler<'a, 'b> {
            buffer: &'a mut Buffer<'b>,
            synth: &'a mut SynthEngine,
            desired_block_size: usize,
        }

        impl<'a, 'b> BlocksHandler<'a, 'b> {
            #[inline]
            fn process_single_block(&mut self, sample_from: usize, samples: usize) {
                self.synth.process(
                    samples,
                    self.buffer
                        .as_slice()
                        .iter_mut()
                        .map(|buff| &mut buff[sample_from..sample_from + samples]),
                );
            }

            fn process(&mut self, mut sample_from: usize, sample_to: usize) -> usize {
                while sample_to - sample_from >= self.desired_block_size {
                    self.process_single_block(sample_from, self.desired_block_size);
                    sample_from += self.desired_block_size;
                }

                sample_from
            }

            #[inline]
            fn process_all(&mut self, mut sample_from: usize, sample_to: usize) -> usize {
                while sample_from < sample_to {
                    let samples = self.desired_block_size.min(sample_to - sample_from);

                    self.process_single_block(sample_from, samples);
                    sample_from += samples;
                }

                sample_from
            }
        }

        let mut synth = self.synth_engine.lock();

        assert_no_alloc::assert_no_alloc(|| {
            let total_samples = buffer.samples();
            let desired_block_size = synth.block_size();

            let mut blocks_handler = BlocksHandler {
                buffer,
                synth: &mut synth,
                desired_block_size,
            };

            let mut events = EventReorderer::new(context);
            let mut next_event = events.next_event();
            let mut sample_from = 0usize;

            while let Some(event) = next_event {
                let sample_to = event.timing() as usize;

                if sample_to > sample_from && event.is_barrier() {
                    sample_from = blocks_handler.process_all(sample_from, sample_to);
                } else if sample_to - sample_from >= desired_block_size {
                    sample_from = blocks_handler.process(sample_from, sample_to);
                }

                Self::process_event(blocks_handler.synth, event);
                next_event = events.next_event();
            }

            blocks_handler.process_all(sample_from, total_samples);
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
