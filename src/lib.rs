#![allow(clippy::new_without_default)]

use const_format::concatcp;
use smallvec::SmallVec;

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
use crate::synth_engine::{Expression, ExternalParamsBlock, SynthEngine};
pub use egui;
use nih_plug::prelude::*;
use std::sync::Arc;

pub struct Additizer {
    params: Arc<AdditizerParams>,
    engine: Option<EngineHandle>,
    factory: Arc<EngineFactory>,
}

impl Default for Additizer {
    fn default() -> Self {
        let params = Arc::new(AdditizerParams::default());

        let external_params = Arc::new(ExternalParamsBlock {
            float_params: [
                params.float_param_1.clone(),
                params.float_param_2.clone(),
                params.float_param_3.clone(),
                params.float_param_4.clone(),
            ],
        });

        let factory = Arc::new(EngineFactory::new(params.volume.clone(), external_params));

        Self {
            params,
            engine: None,
            factory,
        }
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
            NoteEvent::Choke { .. } => 3, // Highest priority
            NoteEvent::NoteOff { .. } => 2,
            NoteEvent::NoteOn { .. } => 1,
            _ => 0, // Lowest priority
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
    fn process_event(synth: &mut SynthEngine, event: NoteEvent<()>, block_start: usize) {
        // log!("Event: {:?}", event);

        match event {
            NoteEvent::NoteOn {
                timing,
                channel,
                note,
                velocity,
                ..
            } => {
                synth.handle_note_on(channel, note, velocity, timing as usize - block_start);
            }
            NoteEvent::NoteOff {
                timing,
                channel,
                note,
                velocity,
                ..
            } => {
                synth.handle_note_off(channel, note, velocity, timing as usize - block_start);
            }
            NoteEvent::Choke { channel, note, .. } => {
                synth.handle_choke(channel, note);
            }
            NoteEvent::PolyVolume {
                timing,
                channel,
                note,
                gain,
                ..
            } => {
                synth.handle_note_expression(
                    channel,
                    note,
                    Expression::Gain,
                    timing as usize - block_start,
                    gain,
                );
            }
            NoteEvent::PolyPan {
                timing,
                channel,
                note,
                pan,
                ..
            } => {
                synth.handle_note_expression(
                    channel,
                    note,
                    Expression::Pan,
                    timing as usize - block_start,
                    pan,
                );
            }
            NoteEvent::PolyTuning {
                timing,
                channel,
                note,
                tuning,
                ..
            } => {
                synth.handle_note_expression(
                    channel,
                    note,
                    Expression::Pitch,
                    timing as usize - block_start,
                    tuning,
                );
            }
            NoteEvent::PolyBrightness {
                timing,
                channel,
                note,
                brightness,
                ..
            } => {
                synth.handle_note_expression(
                    channel,
                    note,
                    Expression::Timbre,
                    timing as usize - block_start,
                    brightness,
                );
            }
            NoteEvent::PolyPressure {
                timing,
                channel,
                note,
                pressure,
                ..
            } => {
                synth.handle_note_expression(
                    channel,
                    note,
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
    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

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

        true
    }

    fn reset(&mut self) {}

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

            let mut events = EventReorderer::new(context);
            let mut next_event = events.next_event();

            for (block_start, block) in buffer.iter_blocks(block_size) {
                let samples = block.samples();
                let sample_to = block_start + samples;

                while let Some(event) =
                    next_event.take_if(|event| (event.timing() as usize) < sample_to)
                {
                    Self::process_event(&mut synth, event, block_start);
                    next_event = events.next_event();
                }

                let mut channels = block.into_iter();
                let mut channel_outputs = [channels.next().unwrap(), channels.next().unwrap()];
                synth.process(samples, update_ui, &mut channel_outputs);
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
