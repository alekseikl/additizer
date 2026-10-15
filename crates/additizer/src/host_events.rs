use crate::params::ExtParam;
use addi_engine::{
    EngineLinks, Expression, Note, Smp, SynthEngine, external_param::NUM_EXT_PARAMS,
};
use nice_plug::midi::{Channel, Key, NoteEvent, VoiceID};

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
        _ => (),
    }
}
