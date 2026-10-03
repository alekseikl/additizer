use super::*;
use crate::synth_engine::StereoSample;

fn slot(src_slot: usize) -> InputSlot {
    InputSlot {
        src_slot,
        modulation_slot: None,
        amount: StereoSample::ONE,
    }
}

#[test]
fn get_scalar_reads_control_output() {
    let mut arena = OutputsArena::new();
    let src_slot = arena.allocate_samples_slot(true);

    assert_eq!(
        arena.get_scalar(&[slot(src_slot)], 0, 0, Some(0)),
        Some(0.0)
    );
}

#[test]
#[should_panic(expected = "audio-rate")]
fn get_scalar_panics_for_audio_output() {
    let mut arena = OutputsArena::new();
    let src_slot = arena.allocate_samples_slot(false);

    let _ = arena.get_scalar(&[slot(src_slot)], 0, 0, Some(0));
}

#[test]
#[should_panic(expected = "audio-rate")]
fn get_scalar_panics_for_audio_rate_modulator() {
    let mut arena = OutputsArena::new();
    let slots = [InputSlot {
        src_slot: arena.allocate_samples_slot(true),
        modulation_slot: Some(arena.allocate_samples_slot(false)),
        amount: StereoSample::ONE,
    }];

    let _ = arena.get_scalar(&slots, 0, 0, Some(0));
}

#[test]
fn reused_slot_updates_rate() {
    let mut arena = OutputsArena::new();
    let src_slot = arena.allocate_samples_slot(false);
    arena.free_samples_slot(src_slot);

    let src_slot = arena.allocate_samples_slot(true);
    assert_eq!(
        arena.get_scalar(&[slot(src_slot)], 0, 0, Some(0)),
        Some(0.0)
    );
}

fn add_buff(arena: &OutputsArena, slots: &[InputSlot], is_control: bool) -> bool {
    let mut result = [0.0; 4];
    arena.add_buff_to(slots, is_control, 0, 0, 0, &mut result)
}

#[test]
fn add_buff_to_reads_control_as_control() {
    let mut arena = OutputsArena::new();
    let src_slot = arena.allocate_samples_slot(true);

    assert!(add_buff(&arena, &[slot(src_slot)], true));
}

#[test]
fn add_buff_to_reads_audio_or_control_as_audio() {
    let mut arena = OutputsArena::new();
    let audio_slot = arena.allocate_samples_slot(false);
    let control_slot = arena.allocate_samples_slot(true);

    assert!(add_buff(&arena, &[slot(audio_slot)], false));
    assert!(add_buff(&arena, &[slot(control_slot)], false));
}

#[test]
#[should_panic(expected = "audio-rate")]
fn add_buff_to_panics_for_audio_as_control() {
    let mut arena = OutputsArena::new();
    let src_slot = arena.allocate_samples_slot(false);

    let _ = add_buff(&arena, &[slot(src_slot)], true);
}

#[test]
#[should_panic(expected = "audio-rate")]
fn add_buff_to_panics_for_audio_rate_modulator_as_control() {
    let mut arena = OutputsArena::new();
    let slots = [InputSlot {
        src_slot: arena.allocate_samples_slot(true),
        modulation_slot: Some(arena.allocate_samples_slot(false)),
        amount: StereoSample::ONE,
    }];

    let _ = add_buff(&arena, &slots, true);
}
