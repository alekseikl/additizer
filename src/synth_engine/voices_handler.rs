use std::{cmp::Reverse, collections::VecDeque};

use smallvec::SmallVec;

use crate::{
    synth_engine::{
        Expression, Sample,
        buffer::{MonoVoicesLayout, new_mono_voices_layout},
        routing::{ExpressionEvent, MAX_VOICES, VoiceEvent},
    },
    utils::{log, note_to_pitch, pitch_to_freq},
};

pub const MAX_AVAILABLE_VOICES: usize = MAX_VOICES - 4;
pub const BAND_LIMIT_FREQUENCY: Sample = 24_000.0;

type VoiceIdx = u8;

#[derive(Debug, Clone, Copy)]
pub struct Note {
    pub channel: u8,
    pub note: u8,
    pub velocity: Sample,
    pub host_id: Option<i32>,
}

impl Note {
    fn is_same(&self, other: &Self) -> bool {
        self.channel == other.channel && self.note == other.note
    }
}

#[derive(Clone, Copy)]
struct PlayingNote {
    note: Note,
    voice_idx: VoiceIdx,
    seq_idx: u32,
}

pub struct DecayingVoice {
    voice_idx: VoiceIdx,
    still_active: bool,
}

impl DecayingVoice {
    pub fn new(voice_idx: VoiceIdx) -> Self {
        Self {
            voice_idx,
            still_active: false,
        }
    }

    pub fn is_done(&self) -> bool {
        !self.still_active
    }

    pub fn index(&self) -> usize {
        self.voice_idx as usize
    }

    pub fn mark_active(&mut self) {
        self.still_active = true;
    }

    pub fn reset(&mut self) {
        self.still_active = false;
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PlayingVoice {
    voice_idx: VoiceIdx,
    note_bandwidth: u16,
    triggered: Option<u16>,
}

impl PlayingVoice {
    fn new(voice_idx: VoiceIdx, note: u8) -> Self {
        let frequency = pitch_to_freq(note_to_pitch(note as f32));

        Self {
            voice_idx,
            note_bandwidth: (BAND_LIMIT_FREQUENCY / frequency).floor() as u16,
            triggered: None,
        }
    }

    pub fn voice_idx(&self) -> usize {
        self.voice_idx as usize
    }

    pub fn note_bandwidth(&self) -> usize {
        self.note_bandwidth as usize
    }

    pub fn triggered(&self) -> Option<usize> {
        self.triggered.map(|offset| offset as usize)
    }
}

pub type DecayingVoices = SmallVec<[DecayingVoice; MAX_VOICES]>;
pub type PlayingVoices = SmallVec<[PlayingVoice; MAX_VOICES]>;

pub struct VoiceEvents {
    events: SmallVec<[VoiceEvent; 6]>,
}

impl VoiceEvents {
    pub fn new() -> Self {
        Self {
            events: SmallVec::new(),
        }
    }

    pub fn events(&self) -> &[VoiceEvent] {
        &self.events
    }

    fn note_to_pitch(note: u8) -> Sample {
        note_to_pitch(note as f32)
    }

    fn reset(
        &mut self,
        voice_idx: VoiceIdx,
        prev_voice_idx: Option<VoiceIdx>,
        note: Note,
        offset: usize,
    ) {
        self.events.push(VoiceEvent::Reset {
            voice_idx: voice_idx as usize,
            prev_voice_idx: prev_voice_idx.map(|idx| idx as usize),
            pitch: Self::note_to_pitch(note.note),
            velocity: note.velocity,
            offset,
        });
    }

    fn update(&mut self, voice_idx: VoiceIdx, note: Note, offset: usize) {
        self.events.push(VoiceEvent::Update {
            voice_idx: voice_idx as usize,
            pitch: Self::note_to_pitch(note.note),
            velocity: note.velocity,
            offset,
        });
    }

    fn release(&mut self, voice_idx: VoiceIdx, velocity: Sample, offset: usize) {
        self.events.push(VoiceEvent::Release {
            voice_idx: voice_idx as usize,
            velocity,
            offset,
        });
    }

    fn kill(&mut self, voice_idx: VoiceIdx, offset: usize) {
        self.events.push(VoiceEvent::Kill {
            voice_idx: voice_idx as usize,
            offset,
        });
    }
}

pub struct VoicesHandlerMetrics {
    pub num_voices: usize,
    pub legato: bool,
    pub waiting: usize,
    pub playing: usize,
    pub releasing: usize,
    pub killing: usize,
}

pub struct VoicesHandler {
    num_voices: usize,
    legato: bool,
    waiting: Vec<Note>,
    playing: VecDeque<PlayingNote>,
    releasing: VecDeque<PlayingNote>,
    killing: VecDeque<PlayingNote>,
    terminate: Vec<Note>,
    free_voices: Vec<VoiceIdx>,
    triggers: MonoVoicesLayout<Option<u16>>,
    seq_idx: u32,
}

impl VoicesHandler {
    pub fn new(num_voices: usize, legato: bool) -> Self {
        Self {
            num_voices: num_voices.clamp(1, MAX_AVAILABLE_VOICES),
            legato,
            waiting: Vec::with_capacity(64),
            playing: VecDeque::with_capacity(MAX_VOICES),
            releasing: VecDeque::with_capacity(MAX_VOICES),
            killing: VecDeque::with_capacity(MAX_VOICES),
            terminate: Vec::with_capacity(64),
            free_voices: (0..(MAX_VOICES as u8)).rev().collect(),
            triggers: new_mono_voices_layout(),
            seq_idx: 0,
        }
    }

    fn grab_and_reset(
        &mut self,
        prev_voice_idx: Option<VoiceIdx>,
        note: Note,
        offset: usize,
        events: &mut VoiceEvents,
    ) {
        let voice_idx = if let Some(voice_idx) = self.free_voices.pop() {
            voice_idx
        } else if let Some(killing) = self.killing.pop_back() {
            self.terminate_killed(killing.note);
            killing.voice_idx
        } else if let Some(releasing) = self.releasing.pop_back() {
            self.terminate.push(releasing.note);
            releasing.voice_idx
        } else if let Some(playing) = self.playing.pop_back() {
            self.waiting.push(playing.note);
            playing.voice_idx
        } else {
            panic!("grab_and_reset(): Note processing error")
        };

        self.playing.push_front(PlayingNote {
            note,
            voice_idx,
            seq_idx: self.seq_idx,
        });
        self.seq_idx = self.seq_idx.wrapping_add(1);
        self.triggers[voice_idx as usize] = Some(offset as u16);
        events.reset(voice_idx, prev_voice_idx, note, offset);
    }

    fn legato(&mut self, voice_idx: VoiceIdx, note: Note, offset: usize, events: &mut VoiceEvents) {
        self.playing.push_front(PlayingNote {
            note,
            voice_idx,
            seq_idx: self.seq_idx,
        });
        self.seq_idx = self.seq_idx.wrapping_add(1);
        events.update(voice_idx, note, offset);
    }

    fn terminate_killed(&mut self, note: Note) {
        if !self.waiting.iter().any(|w| w.is_same(&note)) {
            self.terminate.push(note);
        }
    }

    fn kill(&mut self, playing: PlayingNote, offset: usize, events: &mut VoiceEvents) {
        self.killing.push_front(playing);
        events.kill(playing.voice_idx, offset);
    }

    fn note_on_monophonic(&mut self, new_note: Note, offset: usize, events: &mut VoiceEvents) {
        // Kill playing note on same channel
        if let Some(playing_idx) = self
            .playing
            .iter()
            .position(|playing| playing.note.channel == new_note.channel)
        {
            let playing = self.playing.remove(playing_idx).unwrap();

            self.waiting.push(playing.note);

            if self.legato {
                self.legato(playing.voice_idx, new_note, offset, events);
            } else {
                self.kill(playing, offset, events);
                self.grab_and_reset(Some(playing.voice_idx), new_note, offset, events);
            }
        }
        // Kill releasing note on same channel
        else if let Some(releasing_idx) = self
            .releasing
            .iter()
            .position(|releasing| releasing.note.channel == new_note.channel)
        {
            let releasing = self.releasing.remove(releasing_idx).unwrap();

            self.kill(releasing, offset, events);
            self.grab_and_reset(Some(releasing.voice_idx), new_note, offset, events);
        } else {
            self.grab_and_reset(None, new_note, offset, events);
        }
    }

    fn note_on_polyphonic(&mut self, new_note: Note, offset: usize, events: &mut VoiceEvents) {
        let mut prev_voice_idx = None;

        // Kill same releasing note
        if let Some(idx) = self
            .releasing
            .iter()
            .position(|releasing| releasing.note.is_same(&new_note))
        {
            let releasing = self.releasing.remove(idx).unwrap();

            self.kill(releasing, offset, events);
            prev_voice_idx = Some(releasing.voice_idx);
        }

        // All available voices have been occupied, kill the oldest one
        if self.playing.len() + self.releasing.len() >= self.num_voices {
            if let Some(releasing) = self.releasing.pop_back() {
                self.kill(releasing, offset, events);
            } else if let Some(playing) = self.playing.pop_back() {
                self.waiting.push(playing.note);
                self.kill(playing, offset, events);
            }
        }

        self.grab_and_reset(prev_voice_idx, new_note, offset, events);
    }

    fn note_on_impl(&mut self, new_note: Note, offset: usize, events: &mut VoiceEvents) {
        let monophonic = self.num_voices == 1;

        // Ignore already pressed notes
        if self
            .waiting
            .iter()
            .any(|waiting| waiting.is_same(&new_note))
            || self
                .playing
                .iter()
                .any(|playing| playing.note.is_same(&new_note))
        {
            log!("Already pressed note came: {:?}", new_note);
            return;
        }

        if monophonic {
            self.note_on_monophonic(new_note, offset, events);
        } else {
            self.note_on_polyphonic(new_note, offset, events);
        }
    }

    pub fn reset_triggers(&mut self) {
        self.triggers.fill(None);
    }

    pub fn handle_note_on(&mut self, note: Note, offset: usize, events: &mut VoiceEvents) {
        self.note_on_impl(note, offset, events);
    }

    pub fn handle_note_off(&mut self, note: Note, offset: usize, events: &mut VoiceEvents) {
        let monophonic = self.num_voices == 1;

        // Waiting note lifted - just remove it from the list
        if let Some(waiting_idx) = self
            .waiting
            .iter()
            .position(|waiting| waiting.is_same(&note))
        {
            let waiting = self.waiting.remove(waiting_idx);
            if !self.killing.iter().any(|k| k.note.is_same(&waiting)) {
                self.terminate.push(waiting);
            }
            return;
        }

        let Some(playing_idx) = self
            .playing
            .iter()
            .position(|playing| playing.note.is_same(&note))
        else {
            log!("Unknown note lifted: {:?}", note);
            return;
        };

        let playing = self.playing.remove(playing_idx).unwrap();

        if monophonic
            && self.legato
            && let Some(waiting_idx) = self
                .waiting
                .iter()
                .rposition(|waiting| waiting.channel == note.channel)
        {
            let waiting_note = self.waiting.remove(waiting_idx);

            self.terminate.push(playing.note);
            self.legato(playing.voice_idx, waiting_note, offset, events);
            return;
        }

        self.releasing.push_front(playing);
        events.release(playing.voice_idx, note.velocity, offset);

        if let Some(waiting_note) = self.waiting.pop() {
            self.note_on_impl(waiting_note, offset, events);
        }
    }

    pub fn handle_choke(&mut self, note: Note) {
        let mut found = false;

        if let Some(playing_idx) = self.playing.iter().position(|p| p.note.is_same(&note)) {
            let playing = self.playing.remove(playing_idx).unwrap();

            self.free_voices.push(playing.voice_idx);
            found = true;
        } else if let Some(releasing_idx) =
            self.releasing.iter().position(|r| r.note.is_same(&note))
        {
            let releasing = self.releasing.remove(releasing_idx).unwrap();

            self.free_voices.push(releasing.voice_idx);
            found = true;
        } else if let Some(killing_idx) = self.killing.iter().position(|k| k.note.is_same(&note)) {
            let killing = self.killing.remove(killing_idx).unwrap();

            self.free_voices.push(killing.voice_idx);
            found = true;
        }

        if let Some(waiting_idx) = self.waiting.iter().position(|w| w.is_same(&note)) {
            self.waiting.remove(waiting_idx);
            found = true;
        }

        if found {
            self.terminate.push(note);
        }
    }

    pub fn choke_all_voices(&mut self) {
        for playing in self.playing.drain(..) {
            self.terminate.push(playing.note);
            self.free_voices.push(playing.voice_idx);
        }

        for releasing in self.releasing.drain(..) {
            self.terminate.push(releasing.note);
            self.free_voices.push(releasing.voice_idx);
        }

        for killing in self.killing.drain(..) {
            if !self.waiting.iter().any(|w| w.is_same(&killing.note)) {
                self.terminate.push(killing.note);
            }
            self.free_voices.push(killing.voice_idx);
        }

        for waiting in self.waiting.drain(..) {
            self.terminate.push(waiting);
        }
    }

    pub fn handle_expression(
        &mut self,
        note: Note,
        expression: Expression,
        offset: usize,
        value: Sample,
    ) -> Option<ExpressionEvent> {
        let voice_idx = self
            .playing
            .iter()
            .find(|p| p.note.is_same(&note))
            .map(|p| p.voice_idx)
            .or_else(|| {
                self.releasing
                    .iter()
                    .find(|r| r.note.is_same(&note))
                    .map(|r| r.voice_idx)
            });

        voice_idx.map(|voice_idx| ExpressionEvent {
            voice_idx: voice_idx as usize,
            expression,
            offset,
            value,
        })
    }

    pub fn set_num_voices(&mut self, num_voices: usize) {
        self.num_voices = num_voices.clamp(1, MAX_AVAILABLE_VOICES);
    }

    pub fn set_legato(&mut self, legato: bool) {
        self.legato = legato;
    }

    pub fn get_metrics(&self) -> VoicesHandlerMetrics {
        VoicesHandlerMetrics {
            num_voices: self.num_voices,
            legato: self.legato,
            waiting: self.waiting.len(),
            playing: self.playing.len(),
            releasing: self.releasing.len(),
            killing: self.killing.len(),
        }
    }

    pub fn get_decaying_voices(&self, decaying_voices: &mut DecayingVoices) {
        decaying_voices.extend(
            self.releasing
                .iter()
                .map(|r| DecayingVoice::new(r.voice_idx)),
        );
        decaying_voices.extend(self.killing.iter().map(|k| DecayingVoice::new(k.voice_idx)));
    }

    pub fn update_decaying_voices(
        &mut self,
        decaying_voices: &[DecayingVoice],
        terminated: &mut Vec<Note>,
    ) {
        for decaying in decaying_voices.iter().filter(|d| d.is_done()) {
            if let Some(releasing_idx) = self
                .releasing
                .iter()
                .position(|r| r.voice_idx == decaying.voice_idx)
            {
                let releasing = self.releasing.remove(releasing_idx).unwrap();

                self.terminate.push(releasing.note);
                self.free_voices.push(decaying.voice_idx);
            } else if let Some(killing_idx) = self
                .killing
                .iter()
                .position(|k| k.voice_idx == decaying.voice_idx)
            {
                let killing = self.killing.remove(killing_idx).unwrap();

                self.terminate_killed(killing.note);
                self.free_voices.push(decaying.voice_idx);
            }
        }

        terminated.append(&mut self.terminate);
    }

    pub fn get_playing_voices(&mut self, playing_voices: &mut PlayingVoices) {
        // Latest voice should be first in array
        let mut playing_and_releasing: SmallVec<[(u8, u8, u32); MAX_VOICES]> = SmallVec::new();

        playing_and_releasing.extend(
            self.playing
                .iter()
                .map(|p| (p.voice_idx, p.note.note, p.seq_idx)),
        );
        playing_and_releasing.extend(
            self.releasing
                .iter()
                .map(|p| (p.voice_idx, p.note.note, p.seq_idx)),
        );

        playing_and_releasing.sort_unstable_by_key(|&(_, _, seq_idx)| Reverse(seq_idx));

        playing_voices.extend(
            playing_and_releasing
                .iter()
                .map(|&(voice_idx, note, _)| PlayingVoice::new(voice_idx, note)),
        );
        playing_voices.extend(
            self.killing
                .iter()
                .map(|k| PlayingVoice::new(k.voice_idx, k.note.note)),
        );

        for playing in playing_voices.iter_mut() {
            playing.triggered = self.triggers[playing.voice_idx()];
        }
    }
}

#[cfg(test)]
mod tests;
