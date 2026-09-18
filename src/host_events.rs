use crate::{
    params::ExtParam,
    synth_engine::{Expression, Note, SynthEngine, external_param::NUM_EXT_PARAMS},
    // utils::log,
};
use nice_plug::midi::{Channel, Key, NoteEvent, VoiceID};

fn note_from_event(voice_id: VoiceID, channel: Channel, key: Key, velocity: f32) -> Option<Note> {
    Some(Note {
        channel: channel.number()?,
        note: key.number()?,
        velocity,
        host_id: voice_id.id(),
    })
}

pub fn process_event(
    synth: &mut SynthEngine,
    event: NoteEvent<()>,
    block_start: usize,
    params: &[ExtParam; NUM_EXT_PARAMS],
) {
    // log!("Event: {:?}", event);

    match event {
        NoteEvent::NoteOn {
            timing,
            voice_id,
            channel,
            key,
            velocity,
        } => {
            if let Some(note) = note_from_event(voice_id, channel, key, velocity) {
                synth.handle_note_on(note, timing as usize - block_start);
            }
        }
        NoteEvent::NoteOff {
            timing,
            voice_id,
            channel,
            key,
            velocity,
        } => {
            if let Some(note) = note_from_event(voice_id, channel, key, velocity) {
                synth.handle_note_off(note, timing as usize - block_start);
            }
        }
        NoteEvent::Choke {
            voice_id,
            channel,
            key,
            ..
        } => {
            if let Some(note) = note_from_event(voice_id, channel, key, 0.0) {
                synth.handle_choke(note);
            } else {
                synth.handle_choke_all();
            }
        }
        NoteEvent::PolyVolume {
            timing,
            voice_id,
            channel,
            key,
            gain,
        } => {
            if let Some(note) = note_from_event(voice_id, channel, key, 0.0) {
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
            if let Some(note) = note_from_event(voice_id, channel, key, 0.0) {
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
            if let Some(note) = note_from_event(voice_id, channel, key, 0.0) {
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
            if let Some(note) = note_from_event(voice_id, channel, key, 0.0) {
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
            if let Some(note) = note_from_event(voice_id, channel, key, 0.0) {
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
