use crate::{
    synth_engine::{
        Expression, Note, SynthEngine,
        external_param::{NUM_EXT_PARAMS, ParamValue},
    },
    utils::log,
};
use nice_plug::prelude::NoteEvent;

pub fn process_event(
    synth: &mut SynthEngine,
    event: NoteEvent<()>,
    block_start: usize,
    param_values: &[ParamValue; NUM_EXT_PARAMS],
) {
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
                param_values,
            );
        }
        _ => (),
    }
}
