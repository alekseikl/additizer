use super::*;
use crate::synth_engine::routing::{Expression, VoiceEvent};

fn note_pitch(note: u8) -> Sample {
    VoiceEvents::note_to_pitch(note)
}

fn handler(num_voices: usize) -> VoicesHandler {
    let mut h = VoicesHandler::new(num_voices, false);
    h.set_num_voices(num_voices);
    h
}

fn events() -> VoiceEvents {
    VoiceEvents::new()
}

fn flush_terminated(h: &mut VoicesHandler, decaying: &[DecayingVoice]) -> Vec<Note> {
    let mut terminated = Vec::new();
    h.update_decaying_voices(decaying, &mut terminated);
    terminated
}

fn trigger_indices(ev: &VoiceEvents) -> Vec<usize> {
    ev.events()
        .iter()
        .filter_map(|e| match e {
            VoiceEvent::Reset { voice_idx, .. } => Some(*voice_idx),
            _ => None,
        })
        .collect()
}

fn count_by_kind(ev: &VoiceEvents) -> (usize, usize, usize, usize, usize) {
    let (mut trig, mut upd, mut rel, mut kill, mut expr) = (0, 0, 0, 0, 0);
    for e in ev.events() {
        match e {
            VoiceEvent::Reset { .. } => trig += 1,
            VoiceEvent::Update { .. } => upd += 1,
            VoiceEvent::Release { .. } => rel += 1,
            VoiceEvent::Kill { .. } => kill += 1,
            VoiceEvent::Expression { .. } => expr += 1,
        }
    }
    (trig, upd, rel, kill, expr)
}

// ---- Construction & setters ----

#[test]
fn new_defaults() {
    let h = VoicesHandler::new(1, false);
    let ui = h.get_ui_state();
    assert_eq!(ui.num_voices, 1);
    assert!(!ui.legato);
    assert_eq!(ui.waiting, 0);
    assert_eq!(ui.playing, 0);
    assert_eq!(ui.releasing, 0);
    assert_eq!(ui.killing, 0);
}

#[test]
fn set_num_voices_clamps() {
    let mut h = VoicesHandler::new(1, false);

    h.set_num_voices(0);
    assert_eq!(h.get_ui_state().num_voices, 1);

    h.set_num_voices(999);
    assert_eq!(h.get_ui_state().num_voices, MAX_AVAILABLE_VOICES);

    h.set_num_voices(8);
    assert_eq!(h.get_ui_state().num_voices, 8);
}

#[test]
fn set_legato_toggles() {
    let mut h = VoicesHandler::new(1, false);
    assert!(!h.legato);
    h.set_legato(true);
    assert!(h.legato);
    h.set_legato(false);
    assert!(!h.legato);
}

// ---- VoiceEvents helpers ----

#[test]
fn note_to_pitch_known_values() {
    assert_eq!(VoiceEvents::note_to_pitch(69), 0.0); // A4
    assert_eq!(VoiceEvents::note_to_pitch(81), 1.0); // A5
}

// ---- Polyphonic note-on ----

#[test]
fn poly_single_note_on() {
    let mut h = handler(4);
    let mut ev = events();

    h.handle_note_on(
        Note {
            channel: 0,
            note: 60,
            velocity: 1.0,
            host_id: None,
        },
        0,
        &mut ev,
    );

    let (trig, _, _, _, _) = count_by_kind(&ev);
    assert_eq!(trig, 1);
    match &ev.events()[0] {
        VoiceEvent::Reset {
            prev_voice_idx,
            pitch,
            velocity,
            ..
        } => {
            assert_eq!(*prev_voice_idx, None);
            assert_eq!(*pitch, note_pitch(60));
            assert_eq!(*velocity, 1.0);
        }
        _ => panic!("expected Reset"),
    }
    assert_eq!(h.get_ui_state().playing, 1);
}

#[test]
fn poly_multiple_notes_get_unique_voices() {
    let mut h = handler(4);
    let mut ev = events();

    h.handle_note_on(
        Note {
            channel: 0,
            note: 60,
            velocity: 1.0,
            host_id: None,
        },
        0,
        &mut ev,
    );
    h.handle_note_on(
        Note {
            channel: 0,
            note: 64,
            velocity: 1.0,
            host_id: None,
        },
        0,
        &mut ev,
    );
    h.handle_note_on(
        Note {
            channel: 0,
            note: 67,
            velocity: 1.0,
            host_id: None,
        },
        0,
        &mut ev,
    );

    assert_eq!(h.get_ui_state().playing, 3);

    let mut indices = trigger_indices(&ev);
    let orig_len = indices.len();
    indices.sort();
    indices.dedup();
    assert_eq!(indices.len(), orig_len);
}

#[test]
fn poly_duplicate_note_ignored() {
    let mut h = handler(4);
    let mut ev = events();

    h.handle_note_on(
        Note {
            channel: 0,
            note: 60,
            velocity: 1.0,
            host_id: None,
        },
        0,
        &mut ev,
    );
    h.handle_note_on(
        Note {
            channel: 0,
            note: 60,
            velocity: 1.0,
            host_id: None,
        },
        0,
        &mut ev,
    );

    assert_eq!(ev.events().len(), 1);
    assert_eq!(h.get_ui_state().playing, 1);
}

#[test]
fn poly_voice_stealing_when_full() {
    let mut h = handler(2);
    let mut ev = events();

    h.handle_note_on(
        Note {
            channel: 0,
            note: 60,
            velocity: 1.0,
            host_id: None,
        },
        0,
        &mut ev,
    );
    h.handle_note_on(
        Note {
            channel: 0,
            note: 64,
            velocity: 1.0,
            host_id: None,
        },
        0,
        &mut ev,
    );
    h.handle_note_on(
        Note {
            channel: 0,
            note: 67,
            velocity: 1.0,
            host_id: None,
        },
        0,
        &mut ev,
    );

    let (trig, _, _, kill, _) = count_by_kind(&ev);
    assert_eq!(trig, 3);
    assert_eq!(kill, 1);

    let ui = h.get_ui_state();
    assert_eq!(ui.playing, 2);
    assert_eq!(ui.waiting, 1);
}

#[test]
fn poly_steals_releasing_before_playing() {
    let mut h = handler(2);
    let mut ev = events();

    h.handle_note_on(
        Note {
            channel: 0,
            note: 60,
            velocity: 1.0,
            host_id: None,
        },
        0,
        &mut ev,
    );
    h.handle_note_on(
        Note {
            channel: 0,
            note: 64,
            velocity: 1.0,
            host_id: None,
        },
        0,
        &mut ev,
    );
    h.handle_note_off(
        Note {
            channel: 0,
            note: 60,
            velocity: 1.0,
            host_id: None,
        },
        0,
        &mut ev,
    );

    let ui = h.get_ui_state();
    assert_eq!(ui.playing, 1);
    assert_eq!(ui.releasing, 1);

    h.handle_note_on(
        Note {
            channel: 0,
            note: 67,
            velocity: 1.0,
            host_id: None,
        },
        0,
        &mut ev,
    );

    let ui = h.get_ui_state();
    assert_eq!(ui.playing, 2);
    assert_eq!(ui.waiting, 0);
}

// ---- Polyphonic note-off ----

#[test]
fn poly_note_off_releases() {
    let mut h = handler(4);
    let mut ev = events();

    h.handle_note_on(
        Note {
            channel: 0,
            note: 60,
            velocity: 1.0,
            host_id: None,
        },
        0,
        &mut ev,
    );
    h.handle_note_off(
        Note {
            channel: 0,
            note: 60,
            velocity: 0.5,
            host_id: None,
        },
        0,
        &mut ev,
    );

    let (_, _, rel, _, _) = count_by_kind(&ev);
    assert_eq!(rel, 1);

    let ui = h.get_ui_state();
    assert_eq!(ui.playing, 0);
    assert_eq!(ui.releasing, 1);
}

#[test]
fn poly_note_off_unknown_is_noop() {
    let mut h = handler(4);
    let mut ev = events();

    h.handle_note_off(
        Note {
            channel: 0,
            note: 60,
            velocity: 1.0,
            host_id: None,
        },
        0,
        &mut ev,
    );
    assert!(ev.events().is_empty());
}

#[test]
fn poly_note_off_activates_waiting_note() {
    let mut h = handler(2);
    let mut ev = events();

    h.handle_note_on(
        Note {
            channel: 0,
            note: 60,
            velocity: 1.0,
            host_id: None,
        },
        0,
        &mut ev,
    );
    h.handle_note_on(
        Note {
            channel: 0,
            note: 64,
            velocity: 1.0,
            host_id: None,
        },
        0,
        &mut ev,
    );
    h.handle_note_on(
        Note {
            channel: 0,
            note: 67,
            velocity: 1.0,
            host_id: None,
        },
        0,
        &mut ev,
    );

    assert_eq!(h.get_ui_state().waiting, 1);

    h.handle_note_off(
        Note {
            channel: 0,
            note: 67,
            velocity: 1.0,
            host_id: None,
        },
        0,
        &mut ev,
    );

    let ui = h.get_ui_state();
    assert_eq!(ui.waiting, 0);
    assert_eq!(ui.playing, 2);
}

// ---- Polyphonic re-trigger of releasing note ----

#[test]
fn poly_retrigger_releasing_note() {
    let mut h = handler(4);
    let mut ev = events();

    h.handle_note_on(
        Note {
            channel: 0,
            note: 60,
            velocity: 1.0,
            host_id: None,
        },
        0,
        &mut ev,
    );
    h.handle_note_off(
        Note {
            channel: 0,
            note: 60,
            velocity: 1.0,
            host_id: None,
        },
        0,
        &mut ev,
    );
    assert_eq!(h.get_ui_state().releasing, 1);

    h.handle_note_on(
        Note {
            channel: 0,
            note: 60,
            velocity: 1.0,
            host_id: None,
        },
        0,
        &mut ev,
    );

    let ui = h.get_ui_state();
    assert_eq!(ui.playing, 1);
    assert_eq!(ui.releasing, 0);
}

// ---- Monophonic (no legato) ----

#[test]
fn mono_note_on_replaces_playing() {
    let mut h = handler(1);
    let mut ev = events();

    h.handle_note_on(
        Note {
            channel: 0,
            note: 60,
            velocity: 1.0,
            host_id: None,
        },
        0,
        &mut ev,
    );
    h.handle_note_on(
        Note {
            channel: 0,
            note: 64,
            velocity: 1.0,
            host_id: None,
        },
        0,
        &mut ev,
    );

    let ui = h.get_ui_state();
    assert_eq!(ui.playing, 1);
    assert_eq!(ui.waiting, 1);
}

#[test]
fn mono_no_legato_kills_and_retriggers() {
    let mut h = handler(1);
    let mut ev = events();

    h.handle_note_on(
        Note {
            channel: 0,
            note: 60,
            velocity: 1.0,
            host_id: None,
        },
        0,
        &mut ev,
    );
    h.handle_note_on(
        Note {
            channel: 0,
            note: 64,
            velocity: 1.0,
            host_id: None,
        },
        0,
        &mut ev,
    );

    let (trig, _, _, kill, _) = count_by_kind(&ev);
    assert_eq!(kill, 1);
    assert_eq!(trig, 2);
}

#[test]
fn mono_note_on_kills_releasing_on_same_channel() {
    let mut h = handler(1);
    let mut ev = events();

    h.handle_note_on(
        Note {
            channel: 0,
            note: 60,
            velocity: 1.0,
            host_id: None,
        },
        0,
        &mut ev,
    );
    h.handle_note_off(
        Note {
            channel: 0,
            note: 60,
            velocity: 1.0,
            host_id: None,
        },
        0,
        &mut ev,
    );
    assert_eq!(h.get_ui_state().releasing, 1);

    h.handle_note_on(
        Note {
            channel: 0,
            note: 64,
            velocity: 1.0,
            host_id: None,
        },
        0,
        &mut ev,
    );

    let ui = h.get_ui_state();
    assert_eq!(ui.playing, 1);
    assert_eq!(ui.releasing, 0);
}

// ---- Monophonic legato ----

#[test]
fn mono_legato_updates_instead_of_retriggering() {
    let mut h = handler(1);
    h.set_legato(true);
    let mut ev = events();

    h.handle_note_on(
        Note {
            channel: 0,
            note: 60,
            velocity: 1.0,
            host_id: None,
        },
        0,
        &mut ev,
    );
    h.handle_note_on(
        Note {
            channel: 0,
            note: 64,
            velocity: 0.8,
            host_id: None,
        },
        0,
        &mut ev,
    );

    let (trig, upd, _, kill, _) = count_by_kind(&ev);
    assert_eq!(trig, 1);
    assert_eq!(upd, 1);
    assert_eq!(kill, 0);

    match &ev.events()[1] {
        VoiceEvent::Update { pitch, .. } => {
            assert_eq!(*pitch, note_pitch(64));
        }
        _ => panic!("expected Update"),
    }
}

#[test]
fn mono_legato_note_off_returns_to_previous() {
    let mut h = handler(1);
    h.set_legato(true);
    let mut ev = events();

    h.handle_note_on(
        Note {
            channel: 0,
            note: 60,
            velocity: 1.0,
            host_id: Some(1),
        },
        0,
        &mut ev,
    );
    h.handle_note_on(
        Note {
            channel: 0,
            note: 64,
            velocity: 0.8,
            host_id: Some(2),
        },
        0,
        &mut ev,
    );
    h.handle_note_off(
        Note {
            channel: 0,
            note: 64,
            velocity: 1.0,
            host_id: Some(2),
        },
        0,
        &mut ev,
    );

    let ui = h.get_ui_state();
    assert_eq!(ui.playing, 1);
    assert_eq!(ui.waiting, 0);
    assert_eq!(ui.releasing, 0);

    match ev.events().last().unwrap() {
        VoiceEvent::Update { pitch, .. } => {
            assert_eq!(*pitch, note_pitch(60));
        }
        _ => panic!("expected Update for legato return"),
    }

    let terminated = flush_terminated(&mut h, &[]);
    assert_eq!(terminated.len(), 1);
    assert_eq!(terminated[0].note, 64);
    assert_eq!(terminated[0].host_id, Some(2));
}

#[test]
fn mono_legato_three_notes_unwind() {
    let mut h = handler(1);
    h.set_legato(true);
    let mut ev = events();

    h.handle_note_on(
        Note {
            channel: 0,
            note: 60,
            velocity: 1.0,
            host_id: None,
        },
        0,
        &mut ev,
    );
    h.handle_note_on(
        Note {
            channel: 0,
            note: 64,
            velocity: 1.0,
            host_id: None,
        },
        0,
        &mut ev,
    );
    h.handle_note_on(
        Note {
            channel: 0,
            note: 67,
            velocity: 1.0,
            host_id: None,
        },
        0,
        &mut ev,
    );

    assert_eq!(h.get_ui_state().waiting, 2);

    h.handle_note_off(
        Note {
            channel: 0,
            note: 67,
            velocity: 1.0,
            host_id: None,
        },
        0,
        &mut ev,
    );
    match ev.events().last().unwrap() {
        VoiceEvent::Update { pitch, .. } => assert_eq!(*pitch, note_pitch(64)),
        _ => panic!("expected Update"),
    }

    h.handle_note_off(
        Note {
            channel: 0,
            note: 64,
            velocity: 1.0,
            host_id: None,
        },
        0,
        &mut ev,
    );
    match ev.events().last().unwrap() {
        VoiceEvent::Update { pitch, .. } => assert_eq!(*pitch, note_pitch(60)),
        _ => panic!("expected Update"),
    }

    h.handle_note_off(
        Note {
            channel: 0,
            note: 60,
            velocity: 1.0,
            host_id: None,
        },
        0,
        &mut ev,
    );
    let (_, _, rel, _, _) = count_by_kind(&ev);
    assert!(rel >= 1);
    assert_eq!(h.get_ui_state().playing, 0);
}

// ---- Waiting note removal ----

#[test]
fn note_off_waiting_note_just_removes() {
    let mut h = handler(1);
    let mut ev = events();

    h.handle_note_on(
        Note {
            channel: 0,
            note: 60,
            velocity: 1.0,
            host_id: None,
        },
        0,
        &mut ev,
    );
    h.handle_note_on(
        Note {
            channel: 0,
            note: 64,
            velocity: 1.0,
            host_id: None,
        },
        0,
        &mut ev,
    );
    let before_len = ev.events().len();

    h.handle_note_off(
        Note {
            channel: 0,
            note: 60,
            velocity: 1.0,
            host_id: None,
        },
        0,
        &mut ev,
    );

    assert_eq!(ev.events().len(), before_len);
    assert_eq!(h.get_ui_state().waiting, 0);
}

// ---- handle_choke ----

#[test]
fn choke_playing_note_frees_voice() {
    let mut h = handler(4);
    let mut ev = events();

    h.handle_note_on(
        Note {
            channel: 0,
            note: 60,
            velocity: 1.0,
            host_id: Some(7),
        },
        0,
        &mut ev,
    );
    let free_before = h.free_voices.len();

    h.handle_choke(Note {
        channel: 0,
        note: 60,
        velocity: 0.0,
        host_id: Some(7),
    });

    assert_eq!(h.get_ui_state().playing, 0);
    assert_eq!(h.free_voices.len(), free_before + 1);

    let terminated = flush_terminated(&mut h, &[]);
    assert_eq!(terminated.len(), 1);
    assert_eq!(terminated[0].note, 60);
    assert_eq!(terminated[0].host_id, Some(7));
}

#[test]
fn choke_releasing_note_frees_voice() {
    let mut h = handler(4);
    let mut ev = events();

    h.handle_note_on(
        Note {
            channel: 0,
            note: 60,
            velocity: 1.0,
            host_id: Some(8),
        },
        0,
        &mut ev,
    );
    h.handle_note_off(
        Note {
            channel: 0,
            note: 60,
            velocity: 1.0,
            host_id: Some(8),
        },
        0,
        &mut ev,
    );
    let free_before = h.free_voices.len();

    h.handle_choke(Note {
        channel: 0,
        note: 60,
        velocity: 0.0,
        host_id: Some(8),
    });

    assert_eq!(h.get_ui_state().releasing, 0);
    assert_eq!(h.free_voices.len(), free_before + 1);

    let terminated = flush_terminated(&mut h, &[]);
    assert_eq!(terminated.len(), 1);
    assert_eq!(terminated[0].note, 60);
    assert_eq!(terminated[0].host_id, Some(8));
}

#[test]
fn choke_unknown_is_noop() {
    let mut h = handler(4);
    let free_before = h.free_voices.len();
    h.handle_choke(Note {
        channel: 0,
        note: 60,
        velocity: 0.0,
        host_id: None,
    });
    assert_eq!(h.free_voices.len(), free_before);
    assert!(flush_terminated(&mut h, &[]).is_empty());
}

#[test]
fn choke_stolen_waiting_note_terminates_and_does_not_resound() {
    let mut h = handler(2);
    let mut ev = events();

    h.handle_note_on(
        Note {
            channel: 0,
            note: 60,
            velocity: 1.0,
            host_id: Some(1),
        },
        0,
        &mut ev,
    );
    h.handle_note_on(
        Note {
            channel: 0,
            note: 64,
            velocity: 1.0,
            host_id: Some(2),
        },
        0,
        &mut ev,
    );
    h.handle_note_on(
        Note {
            channel: 0,
            note: 67,
            velocity: 1.0,
            host_id: Some(3),
        },
        0,
        &mut ev,
    );

    assert_eq!(h.get_ui_state().waiting, 1);
    assert_eq!(h.get_ui_state().killing, 1);

    h.handle_choke(Note {
        channel: 0,
        note: 60,
        velocity: 0.0,
        host_id: Some(1),
    });

    assert_eq!(h.get_ui_state().waiting, 0);
    assert_eq!(h.get_ui_state().killing, 0);

    let terminated = flush_terminated(&mut h, &[]);
    assert_eq!(terminated.len(), 1);
    assert_eq!(terminated[0].note, 60);
    assert_eq!(terminated[0].host_id, Some(1));

    h.handle_note_off(
        Note {
            channel: 0,
            note: 67,
            velocity: 1.0,
            host_id: Some(3),
        },
        0,
        &mut ev,
    );

    let ui = h.get_ui_state();
    assert_eq!(ui.waiting, 0);
    assert_eq!(ui.playing, 1);
}

#[test]
fn choke_waiting_only_note_terminates() {
    let mut h = handler(1);
    let mut ev = events();

    h.handle_note_on(
        Note {
            channel: 0,
            note: 60,
            velocity: 1.0,
            host_id: Some(1),
        },
        0,
        &mut ev,
    );
    h.handle_note_on(
        Note {
            channel: 0,
            note: 64,
            velocity: 1.0,
            host_id: Some(2),
        },
        0,
        &mut ev,
    );

    let mut decaying = DecayingVoices::new();
    h.get_decaying_voices(&mut decaying);
    let terminated = flush_terminated(&mut h, &decaying);
    assert!(terminated.is_empty());
    assert_eq!(h.get_ui_state().waiting, 1);
    assert_eq!(h.get_ui_state().killing, 0);

    h.handle_choke(Note {
        channel: 0,
        note: 60,
        velocity: 0.0,
        host_id: Some(1),
    });

    assert_eq!(h.get_ui_state().waiting, 0);
    let terminated = flush_terminated(&mut h, &[]);
    assert_eq!(terminated.len(), 1);
    assert_eq!(terminated[0].note, 60);
    assert_eq!(terminated[0].host_id, Some(1));
}

#[test]
fn choke_killing_only_note_terminates() {
    let mut h = handler(2);
    let mut ev = events();

    h.handle_note_on(
        Note {
            channel: 0,
            note: 60,
            velocity: 1.0,
            host_id: Some(1),
        },
        0,
        &mut ev,
    );
    h.handle_note_on(
        Note {
            channel: 0,
            note: 64,
            velocity: 1.0,
            host_id: Some(2),
        },
        0,
        &mut ev,
    );
    h.handle_note_off(
        Note {
            channel: 0,
            note: 60,
            velocity: 1.0,
            host_id: Some(1),
        },
        0,
        &mut ev,
    );
    h.handle_note_on(
        Note {
            channel: 0,
            note: 67,
            velocity: 1.0,
            host_id: Some(3),
        },
        0,
        &mut ev,
    );

    assert_eq!(h.get_ui_state().killing, 1);
    assert_eq!(h.get_ui_state().waiting, 0);

    h.handle_choke(Note {
        channel: 0,
        note: 60,
        velocity: 0.0,
        host_id: Some(1),
    });

    assert_eq!(h.get_ui_state().killing, 0);
    let terminated = flush_terminated(&mut h, &[]);
    assert_eq!(terminated.len(), 1);
    assert_eq!(terminated[0].note, 60);
    assert_eq!(terminated[0].host_id, Some(1));
}

// ---- handle_expression ----

#[test]
fn expression_on_playing_note() {
    let mut h = handler(4);
    let mut ev = events();

    h.handle_note_on(
        Note {
            channel: 0,
            note: 60,
            velocity: 1.0,
            host_id: None,
        },
        0,
        &mut ev,
    );
    h.handle_expression(
        Note {
            channel: 0,
            note: 60,
            velocity: 0.0,
            host_id: None,
        },
        Expression::Pitch,
        0,
        0.5,
        &mut ev,
    );

    assert_eq!(ev.events().len(), 2);
    match &ev.events()[1] {
        VoiceEvent::Expression {
            expression,
            offset: timing,
            value,
            ..
        } => {
            assert_eq!(*expression, Expression::Pitch);
            assert_eq!(*timing, 0);
            assert_eq!(*value, 0.5);
        }
        _ => panic!("expected Expression event"),
    }
}

#[test]
fn expression_on_releasing_note() {
    let mut h = handler(4);
    let mut ev = events();

    h.handle_note_on(
        Note {
            channel: 0,
            note: 60,
            velocity: 1.0,
            host_id: None,
        },
        0,
        &mut ev,
    );
    h.handle_note_off(
        Note {
            channel: 0,
            note: 60,
            velocity: 1.0,
            host_id: None,
        },
        0,
        &mut ev,
    );
    assert_eq!(h.get_ui_state().releasing, 1);

    let len_before = ev.events().len();
    h.handle_expression(
        Note {
            channel: 0,
            note: 60,
            velocity: 0.0,
            host_id: None,
        },
        Expression::Pressure,
        0,
        0.3,
        &mut ev,
    );
    assert_eq!(ev.events().len(), len_before + 1);
    match &ev.events()[len_before] {
        VoiceEvent::Expression {
            expression,
            offset: timing,
            value,
            ..
        } => {
            assert_eq!(*expression, Expression::Pressure);
            assert_eq!(*timing, 0);
            assert_eq!(*value, 0.3);
        }
        _ => panic!("expected Expression event"),
    }
}

#[test]
fn expression_on_unknown_note_is_noop() {
    let mut h = handler(4);
    let mut ev = events();

    h.handle_expression(
        Note {
            channel: 0,
            note: 60,
            velocity: 0.0,
            host_id: None,
        },
        Expression::Gain,
        0,
        0.5,
        &mut ev,
    );
    assert!(ev.events().is_empty());
}

// ---- DecayingVoice lifecycle ----

#[test]
fn decaying_voice_lifecycle() {
    let mut dv = DecayingVoice::new(5);
    assert_eq!(dv.index(), 5);
    assert!(dv.is_done());

    dv.mark_active();
    assert!(!dv.is_done());

    dv.reset();
    assert!(dv.is_done());
}

// ---- get_decaying_voices ----

#[test]
fn get_decaying_voices_includes_releasing() {
    let mut h = handler(4);
    let mut ev = events();

    h.handle_note_on(
        Note {
            channel: 0,
            note: 60,
            velocity: 1.0,
            host_id: None,
        },
        0,
        &mut ev,
    );
    h.handle_note_off(
        Note {
            channel: 0,
            note: 60,
            velocity: 1.0,
            host_id: None,
        },
        0,
        &mut ev,
    );

    let mut decaying = DecayingVoices::new();
    h.get_decaying_voices(&mut decaying);
    assert_eq!(decaying.len(), 1);
}

#[test]
fn get_decaying_voices_includes_killing() {
    let mut h = handler(1);
    let mut ev = events();

    h.handle_note_on(
        Note {
            channel: 0,
            note: 60,
            velocity: 1.0,
            host_id: None,
        },
        0,
        &mut ev,
    );
    h.handle_note_on(
        Note {
            channel: 0,
            note: 64,
            velocity: 1.0,
            host_id: None,
        },
        0,
        &mut ev,
    );

    let mut decaying = DecayingVoices::new();
    h.get_decaying_voices(&mut decaying);
    assert_eq!(decaying.len(), h.get_ui_state().killing);
}

// ---- update_decaying_voices ----

#[test]
fn update_decaying_voices_frees_done() {
    let mut h = handler(4);
    let mut ev = events();

    h.handle_note_on(
        Note {
            channel: 0,
            note: 60,
            velocity: 1.0,
            host_id: None,
        },
        0,
        &mut ev,
    );
    h.handle_note_off(
        Note {
            channel: 0,
            note: 60,
            velocity: 1.0,
            host_id: None,
        },
        0,
        &mut ev,
    );

    let mut decaying = DecayingVoices::new();
    h.get_decaying_voices(&mut decaying);
    let free_before = h.free_voices.len();

    let terminated = flush_terminated(&mut h, &decaying);

    assert_eq!(h.get_ui_state().releasing, 0);
    assert_eq!(h.free_voices.len(), free_before + 1);
    assert_eq!(terminated.len(), 1);
    assert_eq!(terminated[0].note, 60);
    assert_eq!(terminated[0].channel, 0);
}

#[test]
fn update_decaying_voices_keeps_active() {
    let mut h = handler(4);
    let mut ev = events();

    h.handle_note_on(
        Note {
            channel: 0,
            note: 60,
            velocity: 1.0,
            host_id: None,
        },
        0,
        &mut ev,
    );
    h.handle_note_off(
        Note {
            channel: 0,
            note: 60,
            velocity: 1.0,
            host_id: None,
        },
        0,
        &mut ev,
    );

    let mut decaying = DecayingVoices::new();
    h.get_decaying_voices(&mut decaying);
    decaying[0].mark_active();

    let free_before = h.free_voices.len();
    let terminated = flush_terminated(&mut h, &decaying);

    assert_eq!(h.get_ui_state().releasing, 1);
    assert_eq!(h.free_voices.len(), free_before);
    assert!(terminated.is_empty());
}

#[test]
fn update_decaying_voices_frees_killing() {
    let mut h = handler(1);
    let mut ev = events();

    h.handle_note_on(
        Note {
            channel: 0,
            note: 60,
            velocity: 1.0,
            host_id: None,
        },
        0,
        &mut ev,
    );
    h.handle_note_on(
        Note {
            channel: 0,
            note: 64,
            velocity: 1.0,
            host_id: None,
        },
        0,
        &mut ev,
    );

    let killing_before = h.get_ui_state().killing;
    let free_before = h.free_voices.len();
    let mut decaying = DecayingVoices::new();
    h.get_decaying_voices(&mut decaying);

    let terminated = flush_terminated(&mut h, &decaying);

    assert_eq!(h.get_ui_state().killing, 0);
    assert_eq!(h.free_voices.len(), free_before + killing_before);
    // The killed note is still waiting, so it must not be reported as terminated.
    assert!(terminated.is_empty());
}

#[test]
fn waiting_note_off_terminates() {
    let mut h = handler(2);
    let mut ev = events();

    h.handle_note_on(
        Note {
            channel: 0,
            note: 60,
            velocity: 1.0,
            host_id: Some(1),
        },
        0,
        &mut ev,
    );
    h.handle_note_on(
        Note {
            channel: 0,
            note: 64,
            velocity: 1.0,
            host_id: Some(2),
        },
        0,
        &mut ev,
    );
    h.handle_note_on(
        Note {
            channel: 0,
            note: 67,
            velocity: 1.0,
            host_id: Some(3),
        },
        0,
        &mut ev,
    );
    assert_eq!(h.get_ui_state().waiting, 1);

    h.handle_note_off(
        Note {
            channel: 0,
            note: 60,
            velocity: 1.0,
            host_id: Some(1),
        },
        0,
        &mut ev,
    );

    assert_eq!(h.get_ui_state().waiting, 0);
    assert_eq!(h.get_ui_state().playing, 2);

    let mut decaying = DecayingVoices::new();
    h.get_decaying_voices(&mut decaying);
    let terminated = flush_terminated(&mut h, &decaying);
    assert_eq!(terminated.len(), 1);
    assert_eq!(terminated[0].note, 60);
    assert_eq!(terminated[0].host_id, Some(1));
}

#[test]
fn restored_waiting_note_terminates_when_old_kill_completes() {
    let mut h = handler(1);
    let mut ev = events();

    h.handle_note_on(
        Note {
            channel: 0,
            note: 60,
            velocity: 1.0,
            host_id: Some(1),
        },
        0,
        &mut ev,
    );
    h.handle_note_on(
        Note {
            channel: 0,
            note: 64,
            velocity: 1.0,
            host_id: Some(2),
        },
        0,
        &mut ev,
    );

    h.handle_note_off(
        Note {
            channel: 0,
            note: 64,
            velocity: 1.0,
            host_id: Some(2),
        },
        0,
        &mut ev,
    );

    assert_eq!(h.get_ui_state().playing, 1);
    assert_eq!(h.get_ui_state().waiting, 0);

    let mut decaying = DecayingVoices::new();
    h.get_decaying_voices(&mut decaying);
    let terminated = flush_terminated(&mut h, &decaying);

    assert!(
        terminated
            .iter()
            .any(|n| n.note == 60 && n.host_id == Some(1))
    );
    assert!(
        terminated
            .iter()
            .any(|n| n.note == 64 && n.host_id == Some(2))
    );
}

#[test]
fn stolen_releasing_note_terminates_when_kill_completes() {
    let mut h = handler(2);
    let mut ev = events();

    h.handle_note_on(
        Note {
            channel: 0,
            note: 60,
            velocity: 1.0,
            host_id: Some(1),
        },
        0,
        &mut ev,
    );
    h.handle_note_on(
        Note {
            channel: 0,
            note: 64,
            velocity: 1.0,
            host_id: Some(2),
        },
        0,
        &mut ev,
    );
    h.handle_note_off(
        Note {
            channel: 0,
            note: 60,
            velocity: 1.0,
            host_id: Some(1),
        },
        0,
        &mut ev,
    );
    h.handle_note_on(
        Note {
            channel: 0,
            note: 67,
            velocity: 1.0,
            host_id: Some(3),
        },
        0,
        &mut ev,
    );

    assert_eq!(h.get_ui_state().killing, 1);
    assert_eq!(h.get_ui_state().waiting, 0);

    let mut decaying = DecayingVoices::new();
    h.get_decaying_voices(&mut decaying);
    let terminated = flush_terminated(&mut h, &decaying);

    assert_eq!(terminated.len(), 1);
    assert_eq!(terminated[0].note, 60);
    assert_eq!(terminated[0].host_id, Some(1));
    assert_eq!(h.get_ui_state().killing, 0);
}

#[test]
fn waiting_note_off_after_kill_then_terminates() {
    let mut h = handler(1);
    let mut ev = events();

    h.handle_note_on(
        Note {
            channel: 0,
            note: 60,
            velocity: 1.0,
            host_id: Some(1),
        },
        0,
        &mut ev,
    );
    h.handle_note_on(
        Note {
            channel: 0,
            note: 64,
            velocity: 1.0,
            host_id: Some(2),
        },
        0,
        &mut ev,
    );
    assert_eq!(h.get_ui_state().waiting, 1);
    assert_eq!(h.get_ui_state().killing, 1);

    let mut decaying = DecayingVoices::new();
    h.get_decaying_voices(&mut decaying);
    let terminated = flush_terminated(&mut h, &decaying);
    assert!(terminated.is_empty());
    assert_eq!(h.get_ui_state().killing, 0);

    h.handle_note_off(
        Note {
            channel: 0,
            note: 60,
            velocity: 1.0,
            host_id: Some(1),
        },
        0,
        &mut ev,
    );

    let terminated = flush_terminated(&mut h, &[]);
    assert_eq!(terminated.len(), 1);
    assert_eq!(terminated[0].note, 60);
    assert_eq!(terminated[0].host_id, Some(1));
}

// ---- get_playing_voices ----

#[test]
fn get_playing_voices_includes_all_active() {
    let mut h = handler(4);
    let mut ev = events();

    h.handle_note_on(
        Note {
            channel: 0,
            note: 60,
            velocity: 1.0,
            host_id: None,
        },
        0,
        &mut ev,
    );
    h.handle_note_on(
        Note {
            channel: 0,
            note: 64,
            velocity: 1.0,
            host_id: None,
        },
        0,
        &mut ev,
    );
    h.handle_note_off(
        Note {
            channel: 0,
            note: 60,
            velocity: 1.0,
            host_id: None,
        },
        0,
        &mut ev,
    );

    let mut playing = PlayingVoices::new();
    h.get_playing_voices(&mut playing);

    assert_eq!(playing.len(), 2);
}

#[test]
fn get_playing_voices_includes_killing() {
    let mut h = handler(1);
    let mut ev = events();

    h.handle_note_on(
        Note {
            channel: 0,
            note: 60,
            velocity: 1.0,
            host_id: None,
        },
        0,
        &mut ev,
    );
    h.handle_note_on(
        Note {
            channel: 0,
            note: 64,
            velocity: 1.0,
            host_id: None,
        },
        0,
        &mut ev,
    );

    let mut playing = PlayingVoices::new();
    h.get_playing_voices(&mut playing);

    let ui = h.get_ui_state();
    assert_eq!(playing.len(), ui.playing + ui.releasing + ui.killing);
}

// ---- Cross-channel / edge cases ----

#[test]
fn different_channels_same_note_are_independent() {
    let mut h = handler(4);
    let mut ev = events();

    h.handle_note_on(
        Note {
            channel: 0,
            note: 60,
            velocity: 1.0,
            host_id: None,
        },
        0,
        &mut ev,
    );
    h.handle_note_on(
        Note {
            channel: 1,
            note: 60,
            velocity: 1.0,
            host_id: None,
        },
        0,
        &mut ev,
    );

    assert_eq!(h.get_ui_state().playing, 2);

    h.handle_note_off(
        Note {
            channel: 0,
            note: 60,
            velocity: 1.0,
            host_id: None,
        },
        0,
        &mut ev,
    );
    assert_eq!(h.get_ui_state().playing, 1);
    assert_eq!(h.get_ui_state().releasing, 1);
}

#[test]
fn mono_different_channel_does_not_steal() {
    let mut h = handler(1);
    let mut ev = events();

    h.handle_note_on(
        Note {
            channel: 0,
            note: 60,
            velocity: 1.0,
            host_id: None,
        },
        0,
        &mut ev,
    );
    h.handle_note_on(
        Note {
            channel: 1,
            note: 64,
            velocity: 1.0,
            host_id: None,
        },
        0,
        &mut ev,
    );

    // Channel 1 has no prior note, so it grabs a voice independently
    // (monophonic stealing is per-channel)
    let ui = h.get_ui_state();
    assert!(ui.playing >= 1);
}

#[test]
fn voice_reuse_after_full_lifecycle() {
    let mut h = handler(2);
    let mut ev = events();

    h.handle_note_on(
        Note {
            channel: 0,
            note: 60,
            velocity: 1.0,
            host_id: None,
        },
        0,
        &mut ev,
    );
    h.handle_note_on(
        Note {
            channel: 0,
            note: 64,
            velocity: 1.0,
            host_id: None,
        },
        0,
        &mut ev,
    );
    let free_after_two = h.free_voices.len();

    h.handle_note_off(
        Note {
            channel: 0,
            note: 60,
            velocity: 1.0,
            host_id: None,
        },
        0,
        &mut ev,
    );
    h.handle_note_off(
        Note {
            channel: 0,
            note: 64,
            velocity: 1.0,
            host_id: None,
        },
        0,
        &mut ev,
    );

    let mut decaying = DecayingVoices::new();
    h.get_decaying_voices(&mut decaying);
    let terminated = flush_terminated(&mut h, &decaying);

    assert_eq!(h.free_voices.len(), free_after_two + 2);
    assert_eq!(h.get_ui_state().releasing, 0);
    assert_eq!(h.get_ui_state().playing, 0);
    assert_eq!(terminated.len(), 2);
}

#[test]
fn get_ui_data_reflects_complex_state() {
    let mut h = handler(4);
    h.set_legato(true);
    let mut ev = events();

    h.handle_note_on(
        Note {
            channel: 0,
            note: 60,
            velocity: 1.0,
            host_id: None,
        },
        0,
        &mut ev,
    );
    h.handle_note_on(
        Note {
            channel: 0,
            note: 64,
            velocity: 1.0,
            host_id: None,
        },
        0,
        &mut ev,
    );
    h.handle_note_on(
        Note {
            channel: 0,
            note: 67,
            velocity: 1.0,
            host_id: None,
        },
        0,
        &mut ev,
    );
    h.handle_note_off(
        Note {
            channel: 0,
            note: 67,
            velocity: 1.0,
            host_id: None,
        },
        0,
        &mut ev,
    );

    let ui = h.get_ui_state();
    assert_eq!(ui.num_voices, 4);
    assert!(ui.legato);
    assert!(ui.playing + ui.waiting + ui.releasing + ui.killing > 0);
}
