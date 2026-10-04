use std::mem::size_of;

use super::{VoicesLayout, new_voices_layout, new_voices_layout_with};
use crate::synth_engine::routing::{LEFT_CHANNEL, MAX_VOICES, NUM_CHANNELS, RIGHT_CHANNEL};

fn filled_in_voice_order() -> VoicesLayout<i32> {
    let mut next = 0;

    new_voices_layout_with(|| {
        let value = next;
        next += 1;
        value
    })
}

fn assert_pair_writes(channel_a: usize, voice_a: usize, channel_b: usize, voice_b: usize) {
    let mut layout = new_voices_layout::<i32>();

    {
        let (a, b) = layout.at_pair_mut(channel_a, voice_a, channel_b, voice_b);
        *a = 10;
        *b = 20;
    }

    for voice_idx in 0..MAX_VOICES {
        for channel_idx in 0..NUM_CHANNELS {
            let expected = match (channel_idx, voice_idx) {
                (channel, voice) if channel == channel_a && voice == voice_a => 10,
                (channel, voice) if channel == channel_b && voice == voice_b => 20,
                _ => 0,
            };

            assert_eq!(*layout.at(channel_idx, voice_idx), expected);
        }
    }
}

#[test]
fn new_voices_layout_defaults_every_slot() {
    let layout = new_voices_layout::<i32>();

    for voice_idx in 0..MAX_VOICES {
        for channel_idx in 0..NUM_CHANNELS {
            assert_eq!(*layout.at(channel_idx, voice_idx), 0);
        }
    }
}

#[test]
fn new_voices_layout_with_stores_channels_of_a_voice_together() {
    let layout = filled_in_voice_order();

    for voice_idx in 0..MAX_VOICES {
        for channel_idx in 0..NUM_CHANNELS {
            let expected = (voice_idx * NUM_CHANNELS + channel_idx) as i32;
            assert_eq!(*layout.at(channel_idx, voice_idx), expected);
        }
    }
}

#[test]
fn channels_of_a_voice_are_adjacent() {
    let layout = new_voices_layout::<u32>();
    let left = layout.at(LEFT_CHANNEL, 6) as *const u32 as usize;
    let right = layout.at(RIGHT_CHANNEL, 6) as *const u32 as usize;
    let next_voice = layout.at(LEFT_CHANNEL, 7) as *const u32 as usize;

    assert_eq!(right - left, size_of::<u32>());
    assert_eq!(next_voice - right, size_of::<u32>());
    assert_eq!(
        (layout.at(LEFT_CHANNEL, 1) as *const u32 as usize)
            - (layout.at(LEFT_CHANNEL, 0) as *const u32 as usize),
        NUM_CHANNELS * size_of::<u32>()
    );
}

#[test]
fn at_mut_updates_only_the_addressed_slot() {
    let mut layout = new_voices_layout::<i32>();
    *layout.at_mut(RIGHT_CHANNEL, MAX_VOICES - 1) = 7;

    assert_eq!(*layout.at(RIGHT_CHANNEL, MAX_VOICES - 1), 7);
    assert_eq!(*layout.at(LEFT_CHANNEL, MAX_VOICES - 1), 0);
    assert_eq!(*layout.at(RIGHT_CHANNEL, MAX_VOICES - 2), 0);
}

#[test]
fn channels_at_mut_updates_both_channels_of_one_voice() {
    let mut layout = new_voices_layout::<i32>();

    {
        let [left, right] = layout.channels_at_mut(3);
        *left = 4;
        *right = *left + 1;
    }

    assert_eq!(*layout.at(LEFT_CHANNEL, 3), 4);
    assert_eq!(*layout.at(RIGHT_CHANNEL, 3), 5);
    assert_eq!(*layout.at(LEFT_CHANNEL, 2), 0);
    assert_eq!(*layout.at(RIGHT_CHANNEL, 4), 0);
}

#[test]
fn at_pair_mut_keeps_argument_order() {
    assert_pair_writes(LEFT_CHANNEL, 0, RIGHT_CHANNEL, 0);
    assert_pair_writes(RIGHT_CHANNEL, 4, LEFT_CHANNEL, 4);
    assert_pair_writes(LEFT_CHANNEL, 1, LEFT_CHANNEL, 7);
    assert_pair_writes(RIGHT_CHANNEL, 9, RIGHT_CHANNEL, 2);
    assert_pair_writes(RIGHT_CHANNEL, 0, LEFT_CHANNEL, MAX_VOICES - 1);
    assert_pair_writes(LEFT_CHANNEL, MAX_VOICES - 1, RIGHT_CHANNEL, 0);
}

#[test]
#[should_panic(expected = "slots must differ")]
fn at_pair_mut_rejects_the_same_slot() {
    let mut layout = new_voices_layout::<i32>();
    let _ = layout.at_pair_mut(RIGHT_CHANNEL, 3, RIGHT_CHANNEL, 3);
}
