use std::collections::VecDeque;

use nih_plug::nih_log;
use smallvec::SmallVec;

use crate::{
    synth_engine::{
        Expression, Sample,
        routing::{MAX_VOICES, VoiceEvent},
    },
    utils::note_to_pitch,
};

pub const MAX_AVAILABLE_VOICES: usize = MAX_VOICES - 8;

type VoiceIdx = u8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct NoteId {
    channel: u8,
    note: u8,
}

#[derive(Clone, Copy)]
struct WaitingNote {
    id: NoteId,
    velocity: u8,
}

#[derive(Clone, Copy)]
struct PlayingNote {
    id: NoteId,
    voice_idx: VoiceIdx,
    velocity: u8,
}

struct ReleasingNote {
    id: NoteId,
    voice_idx: VoiceIdx,
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

pub type DecayingVoices = SmallVec<[DecayingVoice; MAX_VOICES]>;
pub type PlayingVoices = SmallVec<[usize; MAX_VOICES]>;

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

    fn to_float_velocity(velocity: u8) -> Sample {
        velocity as Sample / 127.0
    }

    fn restart(
        &mut self,
        voice_idx: VoiceIdx,
        prev_voice_idx: Option<VoiceIdx>,
        note_id: NoteId,
        velocity: u8,
    ) {
        self.events.push(VoiceEvent::Trigger {
            voice_idx: voice_idx as usize,
            prev_voice_idx: prev_voice_idx.map(|idx| idx as usize),
            pitch: Self::note_to_pitch(note_id.note),
            velocity: Self::to_float_velocity(velocity),
        });
    }

    fn update(&mut self, voice_idx: VoiceIdx, note_id: NoteId, velocity: u8) {
        self.events.push(VoiceEvent::Update {
            voice_idx: voice_idx as usize,
            pitch: Self::note_to_pitch(note_id.note),
            velocity: Self::to_float_velocity(velocity),
        });
    }

    fn release(&mut self, voice_idx: VoiceIdx, velocity: u8) {
        self.events.push(VoiceEvent::Release {
            voice_idx: voice_idx as usize,
            velocity: Self::to_float_velocity(velocity),
        });
    }

    fn kill(&mut self, voice_idx: VoiceIdx) {
        self.events.push(VoiceEvent::Kill {
            voice_idx: voice_idx as usize,
        });
    }

    fn expression(&mut self, voice_idx: VoiceIdx, expression: Expression, value: Sample) {
        self.events.push(VoiceEvent::Expression {
            voice_idx: voice_idx as usize,
            expression,
            value,
        });
    }
}

pub struct VoicesHandlerUiData {
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
    waiting_notes: SmallVec<[WaitingNote; 32]>,
    playing_notes: VecDeque<PlayingNote>,
    releasing_notes: VecDeque<ReleasingNote>,
    killing_voices: VecDeque<VoiceIdx>,
    free_voices: SmallVec<[VoiceIdx; MAX_VOICES]>,
}

impl VoicesHandler {
    pub fn new() -> Self {
        Self {
            num_voices: 1,
            legato: false,
            waiting_notes: SmallVec::new(),
            playing_notes: VecDeque::with_capacity(MAX_VOICES),
            releasing_notes: VecDeque::with_capacity(MAX_VOICES),
            killing_voices: VecDeque::with_capacity(MAX_VOICES),
            free_voices: SmallVec::from_iter((0..(MAX_VOICES as u8)).rev()),
        }
    }

    fn to_int_velocity(velocity: f32) -> u8 {
        (velocity * 127.0).round().clamp(1.0, 127.0) as u8
    }

    fn grab_and_restart_voice(
        &mut self,
        prev_voice_idx: Option<VoiceIdx>,
        note: NoteId,
        velocity: u8,
        events: &mut VoiceEvents,
    ) {
        let Some(voice_idx) = self
            .free_voices
            .pop()
            .or_else(|| self.killing_voices.pop_back())
            .or_else(|| self.releasing_notes.pop_back().map(|r| r.voice_idx))
            .or_else(|| {
                self.playing_notes
                    .pop_back()
                    .inspect(|p| {
                        self.waiting_notes.push(WaitingNote {
                            id: p.id,
                            velocity: p.velocity,
                        });
                    })
                    .map(|p| p.voice_idx)
            })
        else {
            panic!("restart_voice(): Note processing error")
        };

        self.playing_notes.push_front(PlayingNote {
            id: note,
            voice_idx,
            velocity,
        });
        events.restart(voice_idx, prev_voice_idx, note, velocity);
    }

    fn apply_legato(
        &mut self,
        voice_idx: VoiceIdx,
        note_id: NoteId,
        velocity: u8,
        events: &mut VoiceEvents,
    ) {
        self.playing_notes.push_front(PlayingNote {
            id: note_id,
            voice_idx,
            velocity,
        });
        events.update(voice_idx, note_id, velocity);
    }

    fn kill_voice(&mut self, voice_idx: VoiceIdx, events: &mut VoiceEvents) {
        self.killing_voices.push_front(voice_idx);
        events.kill(voice_idx);
    }

    fn note_on_monophonic(&mut self, new_note: NoteId, velocity: u8, events: &mut VoiceEvents) {
        // Kill releasing note on same channel
        if let Some(releasing_idx) = self
            .releasing_notes
            .iter()
            .position(|releasing| releasing.id.channel == new_note.channel)
        {
            let voice_idx = self
                .releasing_notes
                .remove(releasing_idx)
                .unwrap()
                .voice_idx;

            self.kill_voice(voice_idx, events);
            self.grab_and_restart_voice(Some(voice_idx), new_note, velocity, events);

        // Kill playing note on same channel
        } else if let Some(playing_idx) = self
            .playing_notes
            .iter()
            .position(|playing| playing.id.channel == new_note.channel)
        {
            let playing = self.playing_notes.remove(playing_idx).unwrap();

            self.waiting_notes.push(WaitingNote {
                id: playing.id,
                velocity: playing.velocity,
            });

            if self.legato {
                self.apply_legato(playing.voice_idx, new_note, velocity, events);
            } else {
                self.kill_voice(playing.voice_idx, events);
                self.grab_and_restart_voice(Some(playing.voice_idx), new_note, velocity, events);
            }
        } else {
            self.grab_and_restart_voice(None, new_note, velocity, events);
        }
    }

    fn note_on_polyphonic(&mut self, new_note: NoteId, velocity: u8, events: &mut VoiceEvents) {
        // Kill same releasing note
        if let Some(idx) = self
            .releasing_notes
            .iter()
            .position(|releasing| releasing.id == new_note)
        {
            let voice_idx = self.releasing_notes.remove(idx).unwrap().voice_idx;

            self.kill_voice(voice_idx, events);
        }

        // All available voices have been occupied, kill the oldest one
        if self.playing_notes.len() + self.releasing_notes.len() >= self.num_voices {
            let Some(voice_idx) = self
                .releasing_notes
                .pop_back()
                .map(|r| r.voice_idx)
                .or_else(|| {
                    self.playing_notes
                        .pop_back()
                        .inspect(|p| {
                            self.waiting_notes.push(WaitingNote {
                                id: p.id,
                                velocity: p.velocity,
                            });
                        })
                        .map(|p| p.voice_idx)
                })
            else {
                panic!("note_on_polyphonic(): Note processing error")
            };

            self.kill_voice(voice_idx, events);
        }

        self.grab_and_restart_voice(None, new_note, velocity, events);
    }

    fn note_on_impl(&mut self, channel: u8, note: u8, velocity: u8, events: &mut VoiceEvents) {
        let new_note = NoteId { channel, note };
        let monophonic = self.num_voices == 1;

        // Ignore already pressed notes
        if self
            .waiting_notes
            .iter()
            .any(|waiting| waiting.id == new_note)
            || self
                .playing_notes
                .iter()
                .any(|playing| playing.id == new_note)
        {
            nih_log!("Already pressed note came: {:?}", new_note);
            return;
        }

        if monophonic {
            self.note_on_monophonic(new_note, velocity, events);
        } else {
            self.note_on_polyphonic(new_note, velocity, events);
        }
    }

    pub fn handle_note_on(
        &mut self,
        channel: u8,
        note: u8,
        velocity: f32,
        events: &mut VoiceEvents,
    ) {
        self.note_on_impl(channel, note, Self::to_int_velocity(velocity), events);
    }

    pub fn handle_note_off(
        &mut self,
        channel: u8,
        note: u8,
        velocity: f32,
        events: &mut VoiceEvents,
    ) {
        let note_id = NoteId { channel, note };
        let velocity = Self::to_int_velocity(velocity);
        let monophonic = self.num_voices == 1;

        // Waiting note lifted - just remove it from the list
        if let Some(waiting_idx) = self
            .waiting_notes
            .iter()
            .position(|waiting| waiting.id == note_id)
        {
            self.waiting_notes.remove(waiting_idx);
            return;
        }

        let Some(playing_idx) = self
            .playing_notes
            .iter()
            .position(|playing| playing.id == note_id)
        else {
            nih_log!("Unknown note lifted: {:?}", note_id);
            return;
        };

        let playing = self.playing_notes.remove(playing_idx).unwrap();

        if monophonic
            && self.legato
            && let Some(waiting_idx) = self
                .waiting_notes
                .iter()
                .rposition(|waiting| waiting.id.channel == note_id.channel)
        {
            let waiting_note = self.waiting_notes.remove(waiting_idx);

            self.apply_legato(
                playing.voice_idx,
                waiting_note.id,
                waiting_note.velocity,
                events,
            );
            return;
        }

        self.releasing_notes.push_front(ReleasingNote {
            id: playing.id,
            voice_idx: playing.voice_idx,
        });
        events.release(playing.voice_idx, velocity);

        if let Some(waiting_note) = self.waiting_notes.pop() {
            self.note_on_impl(
                waiting_note.id.channel,
                waiting_note.id.note,
                waiting_note.velocity,
                events,
            );
        }
    }

    pub fn handle_choke(&mut self, channel: u8, note: u8) {
        let note_id = NoteId { channel, note };

        if let Some(playing_idx) = self.playing_notes.iter().position(|p| p.id == note_id) {
            let voice_idx = self.playing_notes.remove(playing_idx).unwrap().voice_idx;

            self.free_voices.push(voice_idx);
        } else if let Some(releasing_idx) =
            self.releasing_notes.iter().position(|r| r.id == note_id)
        {
            let voice_idx = self
                .releasing_notes
                .remove(releasing_idx)
                .unwrap()
                .voice_idx;

            self.free_voices.push(voice_idx);
        } else if let Some(waiting_idx) = self.waiting_notes.iter().position(|w| w.id == note_id) {
            self.waiting_notes.remove(waiting_idx);
        }
    }

    pub fn handle_expression(
        &mut self,
        channel: u8,
        note: u8,
        expression: Expression,
        value: Sample,
        events: &mut VoiceEvents,
    ) {
        let note_id = NoteId { channel, note };

        if let Some(voice_idx) = self
            .playing_notes
            .iter()
            .find(|p| p.id == note_id)
            .map(|p| p.voice_idx)
        {
            events.expression(voice_idx, expression, value);
        }
    }

    pub fn set_num_voices(&mut self, num_voices: usize) {
        self.num_voices = num_voices.clamp(1, MAX_AVAILABLE_VOICES);
    }

    pub fn set_legato(&mut self, legato: bool) {
        self.legato = legato;
    }

    pub fn get_ui_data(&self) -> VoicesHandlerUiData {
        VoicesHandlerUiData {
            num_voices: self.num_voices,
            legato: self.legato,
            waiting: self.waiting_notes.len(),
            playing: self.playing_notes.len(),
            releasing: self.releasing_notes.len(),
            killing: self.killing_voices.len(),
        }
    }

    pub fn get_decaying_voices(&self, decaying_voices: &mut DecayingVoices) {
        decaying_voices.extend(
            self.releasing_notes
                .iter()
                .map(|r| DecayingVoice::new(r.voice_idx)),
        );
        decaying_voices.extend(self.killing_voices.iter().copied().map(DecayingVoice::new));
    }

    pub fn update_decaying_voices(&mut self, decaying_voices: &[DecayingVoice]) {
        for decaying in decaying_voices.iter().filter(|d| d.is_done()) {
            if let Some(releasing_idx) = self
                .releasing_notes
                .iter()
                .position(|r| r.voice_idx == decaying.voice_idx)
            {
                self.releasing_notes.remove(releasing_idx);
                self.free_voices.push(decaying.voice_idx);
            } else if let Some(killing_idx) = self
                .killing_voices
                .iter()
                .copied()
                .position(|k| k == decaying.voice_idx)
            {
                self.killing_voices.remove(killing_idx);
                self.free_voices.push(decaying.voice_idx);
            }
        }
    }

    pub fn get_playing_voices(&mut self, playing_voices: &mut PlayingVoices) {
        playing_voices.extend(self.playing_notes.iter().map(|p| p.voice_idx as usize));
        playing_voices.extend(self.releasing_notes.iter().map(|r| r.voice_idx as usize));
        playing_voices.extend(self.killing_voices.iter().map(|k| *k as usize));
    }
}

#[cfg(test)]
mod tests;
