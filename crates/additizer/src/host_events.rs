use crate::params::ExtParam;
use addi_engine::{
    EngineLinks, Expression, Note, Smp, SynthEngine, external_param::NUM_EXT_PARAMS,
};
use nice_plug::midi::{Channel, Key, NoteEvent, VoiceID};

/// Song position for the current host buffer.
/// `anchor` is the host-buffer sample where the position applies.
/// `block_offset` is the host-buffer sample where the block being processed starts.
pub(crate) struct HostTransport {
    tempo: f64,
    pos_seconds: f64,
    pos_beats: f64,
    anchor: isize,
    block_offset: usize,
}

impl HostTransport {
    pub(crate) fn new(tempo: f64, pos_seconds: f64, pos_beats: f64) -> Self {
        Self {
            tempo,
            pos_seconds,
            pos_beats,
            anchor: 0,
            block_offset: 0,
        }
    }

    /// Select the host-buffer block about to be processed.
    pub(crate) fn set_block(&mut self, block_start: usize) {
        self.block_offset = block_start;
    }

    fn set(&mut self, timing: u32, tempo: f64, pos_seconds: f64, pos_beats: f64) {
        self.tempo = tempo;
        self.pos_seconds = pos_seconds;
        self.pos_beats = pos_beats;
        self.anchor = timing as isize;
    }
}

impl addi_engine::HostTransport for HostTransport {
    fn tempo(&self) -> f64 {
        self.tempo
    }

    fn pos_seconds(&self) -> f64 {
        self.pos_seconds
    }

    fn pos_beats(&self) -> f64 {
        self.pos_beats
    }

    fn origin(&self) -> isize {
        self.block_offset as isize - self.anchor
    }
}

fn addressed_note(voice_id: VoiceID, channel: Channel, key: Key, velocity: Smp) -> Option<Note> {
    Some(Note {
        channel: channel.number()?,
        note: key.number()?,
        velocity,
        host_id: voice_id.id(),
    })
}

pub fn process_event(
    synth: &mut SynthEngine<impl EngineLinks>,
    event: NoteEvent<()>,
    block_start: usize,
    params: &[ExtParam; NUM_EXT_PARAMS],
    transport: &mut HostTransport,
) {
    match event {
        NoteEvent::NoteOn {
            timing,
            voice_id,
            channel,
            key,
            velocity,
        } => {
            if let (Some(channel), Some(note)) = (channel.number(), key.number()) {
                synth.handle_note_on(
                    Note {
                        channel,
                        note,
                        velocity,
                        host_id: voice_id.id(),
                    },
                    timing as usize - block_start,
                );
            }
        }
        NoteEvent::NoteOff {
            timing,
            voice_id,
            channel,
            key,
            velocity,
        } => {
            if let (Some(channel), Some(note)) = (channel.number(), key.number()) {
                synth.handle_note_off(
                    Note {
                        channel,
                        note,
                        velocity,
                        host_id: voice_id.id(),
                    },
                    timing as usize - block_start,
                );
            }
        }
        NoteEvent::Choke {
            voice_id,
            channel,
            key,
            ..
        } => {
            if let (Some(channel), Some(note)) = (channel.number(), key.number()) {
                synth.handle_choke(Note {
                    channel,
                    note,
                    velocity: 0.0,
                    host_id: voice_id.id(),
                });
            }
        }
        NoteEvent::PolyVolume {
            timing,
            voice_id,
            channel,
            key,
            gain,
        } => {
            if let Some(note) = addressed_note(voice_id, channel, key, 0.0) {
                synth.handle_note_expression(
                    note,
                    Expression::Gain,
                    timing as usize - block_start,
                    gain,
                );
            }
        }
        NoteEvent::PolyPan {
            timing,
            voice_id,
            channel,
            key,
            pan,
        } => {
            if let Some(note) = addressed_note(voice_id, channel, key, 0.0) {
                synth.handle_note_expression(
                    note,
                    Expression::Pan,
                    timing as usize - block_start,
                    pan,
                );
            }
        }
        NoteEvent::PolyTuning {
            timing,
            voice_id,
            channel,
            key,
            tuning,
        } => {
            if let Some(note) = addressed_note(voice_id, channel, key, 0.0) {
                synth.handle_note_expression(
                    note,
                    Expression::Pitch,
                    timing as usize - block_start,
                    tuning,
                );
            }
        }
        NoteEvent::PolyBrightness {
            timing,
            voice_id,
            channel,
            key,
            brightness,
        } => {
            if let Some(note) = addressed_note(voice_id, channel, key, 0.0) {
                synth.handle_note_expression(
                    note,
                    Expression::Timbre,
                    timing as usize - block_start,
                    brightness,
                );
            }
        }
        NoteEvent::PolyPressure {
            timing,
            voice_id,
            channel,
            key,
            pressure,
        } => {
            if let Some(note) = addressed_note(voice_id, channel, key, 0.0) {
                synth.handle_note_expression(
                    note,
                    Expression::Pressure,
                    timing as usize - block_start,
                    pressure,
                );
            }
        }
        NoteEvent::PolyModulation {
            timing,
            voice_id,
            poly_modulation_id,
            normalized_offset,
        } => {
            synth.handle_poly_modulation(
                poly_modulation_id as usize,
                voice_id,
                timing as usize - block_start,
                normalized_offset,
            );
        }
        NoteEvent::MonoAutomation {
            timing,
            poly_modulation_id,
            normalized_value,
        } => {
            synth.handle_mono_automation(
                poly_modulation_id as usize,
                timing as usize - block_start,
                normalized_value,
                params,
            );
        }
        NoteEvent::Transport {
            timing,
            tempo,
            pos_seconds,
            pos_beats,
        } => {
            transport.set(timing, tempo, pos_seconds, pos_beats);
        }
        _ => (),
    }
}
