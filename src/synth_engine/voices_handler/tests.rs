use super::*;
use crate::synth_engine::routing::{Expression, VoiceEvent};

fn note_pitch(note: u8) -> Sample {
    VoiceEvents::note_to_pitch(note)
}

fn float_vel(velocity: u8) -> Sample {
    VoiceEvents::to_float_velocity(velocity)
}

fn handler(num_voices: usize) -> VoicesHandler {
    let mut h = VoicesHandler::new();
    h.set_num_voices(num_voices);
    h
}

fn events() -> VoiceEvents {
    VoiceEvents::new()
}

fn trigger_indices(ev: &VoiceEvents) -> Vec<usize> {
    ev.events()
        .iter()
        .filter_map(|e| match e {
            VoiceEvent::Trigger { voice_idx, .. } => Some(*voice_idx),
            _ => None,
        })
        .collect()
}

fn count_by_kind(ev: &VoiceEvents) -> (usize, usize, usize, usize, usize) {
    let (mut trig, mut upd, mut rel, mut kill, mut expr) = (0, 0, 0, 0, 0);
    for e in ev.events() {
        match e {
            VoiceEvent::Trigger { .. } => trig += 1,
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
    let h = VoicesHandler::new();
    let ui = h.get_ui_data();
    assert_eq!(ui.num_voices, 1);
    assert!(!ui.legato);
    assert_eq!(ui.waiting, 0);
    assert_eq!(ui.playing, 0);
    assert_eq!(ui.releasing, 0);
    assert_eq!(ui.killing, 0);
}

#[test]
fn set_num_voices_clamps() {
    let mut h = VoicesHandler::new();

    h.set_num_voices(0);
    assert_eq!(h.get_ui_data().num_voices, 1);

    h.set_num_voices(999);
    assert_eq!(h.get_ui_data().num_voices, MAX_AVAILABLE_VOICES);

    h.set_num_voices(8);
    assert_eq!(h.get_ui_data().num_voices, 8);
}

#[test]
fn set_legato_toggles() {
    let mut h = VoicesHandler::new();
    assert!(!h.legato);
    h.set_legato(true);
    assert!(h.legato);
    h.set_legato(false);
    assert!(!h.legato);
}

// ---- Velocity conversion ----

#[test]
fn to_int_velocity_boundaries() {
    assert_eq!(VoicesHandler::to_int_velocity(1.0), 127);
    assert_eq!(VoicesHandler::to_int_velocity(0.0), 1);
    assert_eq!(VoicesHandler::to_int_velocity(0.5), 64);
}

// ---- VoiceEvents helpers ----

#[test]
fn note_to_pitch_known_values() {
    assert_eq!(VoiceEvents::note_to_pitch(69), 0.0); // A4
    assert_eq!(VoiceEvents::note_to_pitch(81), 1.0); // A5
}

#[test]
fn to_float_velocity_known_values() {
    assert_eq!(VoiceEvents::to_float_velocity(127), 1.0);
    assert_eq!(VoiceEvents::to_float_velocity(0), 0.0);
}

// ---- Polyphonic note-on ----

#[test]
fn poly_single_note_on() {
    let mut h = handler(4);
    let mut ev = events();

    h.handle_note_on(0, 60, 1.0, &mut ev);

    let (trig, _, _, _, _) = count_by_kind(&ev);
    assert_eq!(trig, 1);
    match &ev.events()[0] {
        VoiceEvent::Trigger {
            prev_voice_idx,
            pitch,
            velocity,
            ..
        } => {
            assert_eq!(*prev_voice_idx, None);
            assert_eq!(*pitch, note_pitch(60));
            assert_eq!(*velocity, float_vel(127));
        }
        _ => panic!("expected Trigger"),
    }
    assert_eq!(h.get_ui_data().playing, 1);
}

#[test]
fn poly_multiple_notes_get_unique_voices() {
    let mut h = handler(4);
    let mut ev = events();

    h.handle_note_on(0, 60, 1.0, &mut ev);
    h.handle_note_on(0, 64, 1.0, &mut ev);
    h.handle_note_on(0, 67, 1.0, &mut ev);

    assert_eq!(h.get_ui_data().playing, 3);

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

    h.handle_note_on(0, 60, 1.0, &mut ev);
    h.handle_note_on(0, 60, 1.0, &mut ev);

    assert_eq!(ev.events().len(), 1);
    assert_eq!(h.get_ui_data().playing, 1);
}

#[test]
fn poly_voice_stealing_when_full() {
    let mut h = handler(2);
    let mut ev = events();

    h.handle_note_on(0, 60, 1.0, &mut ev);
    h.handle_note_on(0, 64, 1.0, &mut ev);
    h.handle_note_on(0, 67, 1.0, &mut ev);

    let (trig, _, _, kill, _) = count_by_kind(&ev);
    assert_eq!(trig, 3);
    assert_eq!(kill, 1);

    let ui = h.get_ui_data();
    assert_eq!(ui.playing, 2);
    assert_eq!(ui.waiting, 1);
}

#[test]
fn poly_steals_releasing_before_playing() {
    let mut h = handler(2);
    let mut ev = events();

    h.handle_note_on(0, 60, 1.0, &mut ev);
    h.handle_note_on(0, 64, 1.0, &mut ev);
    h.handle_note_off(0, 60, 1.0, &mut ev);

    let ui = h.get_ui_data();
    assert_eq!(ui.playing, 1);
    assert_eq!(ui.releasing, 1);

    h.handle_note_on(0, 67, 1.0, &mut ev);

    let ui = h.get_ui_data();
    assert_eq!(ui.playing, 2);
    assert_eq!(ui.waiting, 0);
}

// ---- Polyphonic note-off ----

#[test]
fn poly_note_off_releases() {
    let mut h = handler(4);
    let mut ev = events();

    h.handle_note_on(0, 60, 1.0, &mut ev);
    h.handle_note_off(0, 60, 0.5, &mut ev);

    let (_, _, rel, _, _) = count_by_kind(&ev);
    assert_eq!(rel, 1);

    let ui = h.get_ui_data();
    assert_eq!(ui.playing, 0);
    assert_eq!(ui.releasing, 1);
}

#[test]
fn poly_note_off_unknown_is_noop() {
    let mut h = handler(4);
    let mut ev = events();

    h.handle_note_off(0, 60, 1.0, &mut ev);
    assert!(ev.events().is_empty());
}

#[test]
fn poly_note_off_activates_waiting_note() {
    let mut h = handler(2);
    let mut ev = events();

    h.handle_note_on(0, 60, 1.0, &mut ev);
    h.handle_note_on(0, 64, 1.0, &mut ev);
    h.handle_note_on(0, 67, 1.0, &mut ev);

    assert_eq!(h.get_ui_data().waiting, 1);

    h.handle_note_off(0, 67, 1.0, &mut ev);

    let ui = h.get_ui_data();
    assert_eq!(ui.waiting, 0);
    assert_eq!(ui.playing, 2);
}

// ---- Polyphonic re-trigger of releasing note ----

#[test]
fn poly_retrigger_releasing_note() {
    let mut h = handler(4);
    let mut ev = events();

    h.handle_note_on(0, 60, 1.0, &mut ev);
    h.handle_note_off(0, 60, 1.0, &mut ev);
    assert_eq!(h.get_ui_data().releasing, 1);

    h.handle_note_on(0, 60, 1.0, &mut ev);

    let ui = h.get_ui_data();
    assert_eq!(ui.playing, 1);
    assert_eq!(ui.releasing, 0);
}

// ---- Monophonic (no legato) ----

#[test]
fn mono_note_on_replaces_playing() {
    let mut h = handler(1);
    let mut ev = events();

    h.handle_note_on(0, 60, 1.0, &mut ev);
    h.handle_note_on(0, 64, 1.0, &mut ev);

    let ui = h.get_ui_data();
    assert_eq!(ui.playing, 1);
    assert_eq!(ui.waiting, 1);
}

#[test]
fn mono_no_legato_kills_and_retriggers() {
    let mut h = handler(1);
    let mut ev = events();

    h.handle_note_on(0, 60, 1.0, &mut ev);
    h.handle_note_on(0, 64, 1.0, &mut ev);

    let (trig, _, _, kill, _) = count_by_kind(&ev);
    assert_eq!(kill, 1);
    assert_eq!(trig, 2);
}

#[test]
fn mono_note_on_kills_releasing_on_same_channel() {
    let mut h = handler(1);
    let mut ev = events();

    h.handle_note_on(0, 60, 1.0, &mut ev);
    h.handle_note_off(0, 60, 1.0, &mut ev);
    assert_eq!(h.get_ui_data().releasing, 1);

    h.handle_note_on(0, 64, 1.0, &mut ev);

    let ui = h.get_ui_data();
    assert_eq!(ui.playing, 1);
    assert_eq!(ui.releasing, 0);
}

// ---- Monophonic legato ----

#[test]
fn mono_legato_updates_instead_of_retriggering() {
    let mut h = handler(1);
    h.set_legato(true);
    let mut ev = events();

    h.handle_note_on(0, 60, 1.0, &mut ev);
    h.handle_note_on(0, 64, 0.8, &mut ev);

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

    h.handle_note_on(0, 60, 1.0, &mut ev);
    h.handle_note_on(0, 64, 0.8, &mut ev);
    h.handle_note_off(0, 64, 1.0, &mut ev);

    let ui = h.get_ui_data();
    assert_eq!(ui.playing, 1);
    assert_eq!(ui.waiting, 0);
    assert_eq!(ui.releasing, 0);

    match ev.events().last().unwrap() {
        VoiceEvent::Update { pitch, .. } => {
            assert_eq!(*pitch, note_pitch(60));
        }
        _ => panic!("expected Update for legato return"),
    }
}

#[test]
fn mono_legato_three_notes_unwind() {
    let mut h = handler(1);
    h.set_legato(true);
    let mut ev = events();

    h.handle_note_on(0, 60, 1.0, &mut ev);
    h.handle_note_on(0, 64, 1.0, &mut ev);
    h.handle_note_on(0, 67, 1.0, &mut ev);

    assert_eq!(h.get_ui_data().waiting, 2);

    h.handle_note_off(0, 67, 1.0, &mut ev);
    match ev.events().last().unwrap() {
        VoiceEvent::Update { pitch, .. } => assert_eq!(*pitch, note_pitch(64)),
        _ => panic!("expected Update"),
    }

    h.handle_note_off(0, 64, 1.0, &mut ev);
    match ev.events().last().unwrap() {
        VoiceEvent::Update { pitch, .. } => assert_eq!(*pitch, note_pitch(60)),
        _ => panic!("expected Update"),
    }

    h.handle_note_off(0, 60, 1.0, &mut ev);
    let (_, _, rel, _, _) = count_by_kind(&ev);
    assert!(rel >= 1);
    assert_eq!(h.get_ui_data().playing, 0);
}

// ---- Waiting note removal ----

#[test]
fn note_off_waiting_note_just_removes() {
    let mut h = handler(1);
    let mut ev = events();

    h.handle_note_on(0, 60, 1.0, &mut ev);
    h.handle_note_on(0, 64, 1.0, &mut ev);
    let before_len = ev.events().len();

    h.handle_note_off(0, 60, 1.0, &mut ev);

    assert_eq!(ev.events().len(), before_len);
    assert_eq!(h.get_ui_data().waiting, 0);
}

// ---- handle_choke ----

#[test]
fn choke_playing_note_frees_voice() {
    let mut h = handler(4);
    let mut ev = events();

    h.handle_note_on(0, 60, 1.0, &mut ev);
    let free_before = h.free_voices.len();

    h.handle_choke(0, 60);

    assert_eq!(h.get_ui_data().playing, 0);
    assert_eq!(h.free_voices.len(), free_before + 1);
}

#[test]
fn choke_releasing_note_frees_voice() {
    let mut h = handler(4);
    let mut ev = events();

    h.handle_note_on(0, 60, 1.0, &mut ev);
    h.handle_note_off(0, 60, 1.0, &mut ev);
    let free_before = h.free_voices.len();

    h.handle_choke(0, 60);

    assert_eq!(h.get_ui_data().releasing, 0);
    assert_eq!(h.free_voices.len(), free_before + 1);
}

#[test]
fn choke_unknown_is_noop() {
    let mut h = handler(4);
    let free_before = h.free_voices.len();
    h.handle_choke(0, 60);
    assert_eq!(h.free_voices.len(), free_before);
}

// ---- handle_expression ----

#[test]
fn expression_on_playing_note() {
    let mut h = handler(4);
    let mut ev = events();

    h.handle_note_on(0, 60, 1.0, &mut ev);
    h.handle_expression(0, 60, Expression::Pitch, 0.5, &mut ev);

    assert_eq!(ev.events().len(), 2);
    match &ev.events()[1] {
        VoiceEvent::Expression {
            expression, value, ..
        } => {
            assert_eq!(*expression, Expression::Pitch);
            assert_eq!(*value, 0.5);
        }
        _ => panic!("expected Expression event"),
    }
}

#[test]
fn expression_on_releasing_note() {
    let mut h = handler(4);
    let mut ev = events();

    h.handle_note_on(0, 60, 1.0, &mut ev);
    h.handle_note_off(0, 60, 1.0, &mut ev);
    h.handle_expression(0, 60, Expression::Pressure, 0.3, &mut ev);

    match ev.events().last().unwrap() {
        VoiceEvent::Expression {
            expression, value, ..
        } => {
            assert_eq!(*expression, Expression::Pressure);
            assert!((value - 0.3).abs() < f32::EPSILON);
        }
        _ => panic!("expected Expression event"),
    }
}

#[test]
fn expression_on_unknown_note_is_noop() {
    let mut h = handler(4);
    let mut ev = events();

    h.handle_expression(0, 60, Expression::Gain, 0.5, &mut ev);
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

    h.handle_note_on(0, 60, 1.0, &mut ev);
    h.handle_note_off(0, 60, 1.0, &mut ev);

    let mut decaying = DecayingVoices::new();
    h.get_decaying_voices(&mut decaying);
    assert_eq!(decaying.len(), 1);
}

#[test]
fn get_decaying_voices_includes_killing() {
    let mut h = handler(1);
    let mut ev = events();

    h.handle_note_on(0, 60, 1.0, &mut ev);
    h.handle_note_on(0, 64, 1.0, &mut ev);

    let mut decaying = DecayingVoices::new();
    h.get_decaying_voices(&mut decaying);
    assert_eq!(decaying.len(), h.get_ui_data().killing);
}

// ---- update_decaying_voices ----

#[test]
fn update_decaying_voices_frees_done() {
    let mut h = handler(4);
    let mut ev = events();

    h.handle_note_on(0, 60, 1.0, &mut ev);
    h.handle_note_off(0, 60, 1.0, &mut ev);

    let mut decaying = DecayingVoices::new();
    h.get_decaying_voices(&mut decaying);
    let free_before = h.free_voices.len();

    h.update_decaying_voices(&decaying);

    assert_eq!(h.get_ui_data().releasing, 0);
    assert_eq!(h.free_voices.len(), free_before + 1);
}

#[test]
fn update_decaying_voices_keeps_active() {
    let mut h = handler(4);
    let mut ev = events();

    h.handle_note_on(0, 60, 1.0, &mut ev);
    h.handle_note_off(0, 60, 1.0, &mut ev);

    let mut decaying = DecayingVoices::new();
    h.get_decaying_voices(&mut decaying);
    decaying[0].mark_active();

    let free_before = h.free_voices.len();
    h.update_decaying_voices(&decaying);

    assert_eq!(h.get_ui_data().releasing, 1);
    assert_eq!(h.free_voices.len(), free_before);
}

#[test]
fn update_decaying_voices_frees_killing() {
    let mut h = handler(1);
    let mut ev = events();

    h.handle_note_on(0, 60, 1.0, &mut ev);
    h.handle_note_on(0, 64, 1.0, &mut ev);

    let killing_before = h.get_ui_data().killing;
    let free_before = h.free_voices.len();
    let mut decaying = DecayingVoices::new();
    h.get_decaying_voices(&mut decaying);

    h.update_decaying_voices(&decaying);

    assert_eq!(h.get_ui_data().killing, 0);
    assert_eq!(h.free_voices.len(), free_before + killing_before);
}

// ---- get_playing_voices ----

#[test]
fn get_playing_voices_includes_all_active() {
    let mut h = handler(4);
    let mut ev = events();

    h.handle_note_on(0, 60, 1.0, &mut ev);
    h.handle_note_on(0, 64, 1.0, &mut ev);
    h.handle_note_off(0, 60, 1.0, &mut ev);

    let mut playing = PlayingVoices::new();
    h.get_playing_voices(&mut playing);

    assert_eq!(playing.len(), 2);
}

#[test]
fn get_playing_voices_includes_killing() {
    let mut h = handler(1);
    let mut ev = events();

    h.handle_note_on(0, 60, 1.0, &mut ev);
    h.handle_note_on(0, 64, 1.0, &mut ev);

    let mut playing = PlayingVoices::new();
    h.get_playing_voices(&mut playing);

    let ui = h.get_ui_data();
    assert_eq!(playing.len(), ui.playing + ui.releasing + ui.killing);
}

// ---- Cross-channel / edge cases ----

#[test]
fn different_channels_same_note_are_independent() {
    let mut h = handler(4);
    let mut ev = events();

    h.handle_note_on(0, 60, 1.0, &mut ev);
    h.handle_note_on(1, 60, 1.0, &mut ev);

    assert_eq!(h.get_ui_data().playing, 2);

    h.handle_note_off(0, 60, 1.0, &mut ev);
    assert_eq!(h.get_ui_data().playing, 1);
    assert_eq!(h.get_ui_data().releasing, 1);
}

#[test]
fn mono_different_channel_does_not_steal() {
    let mut h = handler(1);
    let mut ev = events();

    h.handle_note_on(0, 60, 1.0, &mut ev);
    h.handle_note_on(1, 64, 1.0, &mut ev);

    // Channel 1 has no prior note, so it grabs a voice independently
    // (monophonic stealing is per-channel)
    let ui = h.get_ui_data();
    assert!(ui.playing >= 1);
}

#[test]
fn voice_reuse_after_full_lifecycle() {
    let mut h = handler(2);
    let mut ev = events();

    h.handle_note_on(0, 60, 1.0, &mut ev);
    h.handle_note_on(0, 64, 1.0, &mut ev);
    let free_after_two = h.free_voices.len();

    h.handle_note_off(0, 60, 1.0, &mut ev);
    h.handle_note_off(0, 64, 1.0, &mut ev);

    let mut decaying = DecayingVoices::new();
    h.get_decaying_voices(&mut decaying);
    h.update_decaying_voices(&decaying);

    assert_eq!(h.free_voices.len(), free_after_two + 2);
    assert_eq!(h.get_ui_data().releasing, 0);
    assert_eq!(h.get_ui_data().playing, 0);
}

#[test]
fn get_ui_data_reflects_complex_state() {
    let mut h = handler(4);
    h.set_legato(true);
    let mut ev = events();

    h.handle_note_on(0, 60, 1.0, &mut ev);
    h.handle_note_on(0, 64, 1.0, &mut ev);
    h.handle_note_on(0, 67, 1.0, &mut ev);
    h.handle_note_off(0, 67, 1.0, &mut ev);

    let ui = h.get_ui_data();
    assert_eq!(ui.num_voices, 4);
    assert!(ui.legato);
    assert!(ui.playing + ui.waiting + ui.releasing + ui.killing > 0);
}
