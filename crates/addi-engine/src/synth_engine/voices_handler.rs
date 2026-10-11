use smallvec::SmallVec;

use crate::synth_engine::{
    Expression, Sample,
    buffer::{MonoVoicesLayout, new_mono_voices_layout},
    note_to_pitch,
    routing::{ExpressionEvent, MAX_VOICES, PrevNote, VoiceEvent},
};

pub const MAX_AVAILABLE_VOICES: usize = MAX_VOICES - 4;
pub const BAND_LIMIT_FREQUENCY: Sample = 24_000.0;
const NUM_MIDI_CHANNELS: usize = 16;
const MAX_MIDI_CHANNEL: u8 = NUM_MIDI_CHANNELS as u8 - 1;

/// Slots are grouped as `(0, 1)`, `(2, 3)`, … so modules can process a pair as SIMD lanes.
const VOICE_PAIR_LEN: usize = 2;
const _: () = assert!(MAX_VOICES.is_multiple_of(VOICE_PAIR_LEN));
const _: () = assert!(MAX_VOICES <= u8::MAX as usize);

#[derive(Debug, Clone, Copy)]
pub struct Note {
    pub channel: u8,
    pub note: u8,
    pub velocity: Sample,
    pub host_id: Option<i32>,
}

impl Note {
    fn is_same_note(&self, other: &Self) -> bool {
        self.channel == other.channel && self.note == other.note
    }

    fn is_same_instance(&self, other: &Self) -> bool {
        self.is_same_note(other) && self.host_id == other.host_id
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    NoteOn,
    Release,
    Kill,
}

#[derive(Clone, Copy)]
struct PlayingNote {
    note: Note,
    seq_idx: u32,
    phase: Phase,
}

pub struct DecayingVoice {
    voice_idx: u8,
    still_active: bool,
}

impl DecayingVoice {
    pub fn new(voice_idx: u8) -> Self {
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
    voice_idx: u8,
    note: u8,
    triggered: Option<u16>,
}

impl PlayingVoice {
    pub fn new(voice_idx: u8, note: u8) -> Self {
        Self {
            voice_idx,
            note,
            triggered: None,
        }
    }

    pub fn voice_idx(&self) -> usize {
        self.voice_idx as usize
    }

    pub fn note(&self) -> u8 {
        self.note
    }

    pub fn triggered(&self) -> Option<usize> {
        self.triggered.map(|offset| offset as usize)
    }
}

pub type DecayingVoices = SmallVec<[DecayingVoice; MAX_VOICES]>;
pub type PlayingVoices = SmallVec<[PlayingVoice; MAX_VOICES]>;

pub struct VoiceEvents {
    events: SmallVec<[VoiceEvent; 4]>,
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
        voice_idx: usize,
        replaced_idx: Option<usize>,
        prev_note: Option<PrevNote>,
        note: Note,
        offset: usize,
    ) {
        self.events.push(VoiceEvent::Reset {
            voice_idx,
            replaced_voice_idx: replaced_idx,
            prev_note,
            pitch: Self::note_to_pitch(note.note),
            velocity: note.velocity,
            offset,
        });
    }

    fn update(&mut self, voice_idx: usize, note: Note, offset: usize) {
        self.events.push(VoiceEvent::Update {
            voice_idx,
            pitch: Self::note_to_pitch(note.note),
            velocity: note.velocity,
            offset,
        });
    }

    fn release(&mut self, voice_idx: usize, velocity: Sample, offset: usize) {
        self.events.push(VoiceEvent::Release {
            voice_idx,
            velocity,
            offset,
        });
    }

    fn kill(&mut self, voice_idx: usize, offset: usize) {
        self.events.push(VoiceEvent::Kill { voice_idx, offset });
    }
}

pub struct VoicesHandlerMetrics {
    pub num_voices: usize,
    pub legato: bool,
    pub kill_same_note: bool,
    pub waiting: usize,
    pub playing: usize,
    pub releasing: usize,
    pub killing: usize,
}

pub struct VoicesHandler {
    num_voices: usize,
    legato: bool,
    kill_same_note: bool,
    waiting: Vec<Note>,
    voices: [Option<PlayingNote>; MAX_VOICES],
    terminate: Vec<Note>,
    prev_notes: [Option<u8>; NUM_MIDI_CHANNELS],
    triggers: MonoVoicesLayout<Option<u16>>,
    seq_idx: u32,
}

impl VoicesHandler {
    pub fn new(num_voices: usize, legato: bool, kill_same_note: bool) -> Self {
        Self {
            num_voices: num_voices.clamp(1, MAX_AVAILABLE_VOICES),
            legato,
            kill_same_note,
            waiting: Vec::with_capacity(64),
            voices: [None; MAX_VOICES],
            terminate: Vec::with_capacity(64),
            prev_notes: [None; NUM_MIDI_CHANNELS],
            triggers: new_mono_voices_layout(),
            seq_idx: 0,
        }
    }

    /// Prefer the free slot of a pair that already has one voice, so both lanes stay occupied.
    /// Otherwise take the first slot of the first fully free pair.
    fn alloc_voice(&self) -> Option<usize> {
        let mut free_pair = None;
        let (pairs, _) = self.voices.as_chunks::<VOICE_PAIR_LEN>();

        for (pair_idx, [left, right]) in pairs.iter().enumerate() {
            let base = pair_idx * VOICE_PAIR_LEN;

            match (left.is_some(), right.is_some()) {
                (true, false) => return Some(base + 1),
                (false, true) => return Some(base),
                (false, false) if free_pair.is_none() => free_pair = Some(base),
                _ => {}
            }
        }

        free_pair
    }

    fn count_phase(&self, phase: Phase) -> usize {
        self.voices
            .iter()
            .flatten()
            .filter(|note| note.phase == phase)
            .count()
    }

    fn any_phase(&self, phase: Phase, pred: impl Fn(&PlayingNote) -> bool) -> bool {
        self.voices
            .iter()
            .flatten()
            .any(|note| note.phase == phase && pred(note))
    }

    fn find_phase(
        &self,
        phase: Phase,
        pred: impl Fn(&PlayingNote) -> bool,
    ) -> Option<(usize, PlayingNote)> {
        self.voices.iter().enumerate().find_map(|(idx, slot)| {
            let note = *slot.as_ref()?;
            (note.phase == phase && pred(&note)).then_some((idx, note))
        })
    }

    fn find_phase_note(&self, phase: Phase, note: &Note) -> Option<(usize, PlayingNote)> {
        self.find_phase(phase, |playing| playing.note.is_same_note(note))
    }

    fn find_phase_channel(&self, phase: Phase, channel: u8) -> Option<(usize, PlayingNote)> {
        self.find_phase(phase, |playing| playing.note.channel == channel)
    }

    fn find_phase_any(&self, phase: Phase) -> Option<(usize, PlayingNote)> {
        self.find_phase(phase, |_| true)
    }

    /// Oldest voice in `phase`, by allocation order (`seq_idx`).
    fn find_phase_oldest(&self, phase: Phase) -> Option<(usize, PlayingNote)> {
        self.voices
            .iter()
            .enumerate()
            .filter_map(|(idx, slot)| {
                let note = *slot.as_ref()?;
                (note.phase == phase).then_some((idx, note))
            })
            .min_by_key(|(_, note)| note.seq_idx)
    }

    fn find_active(&self, pred: impl Fn(&PlayingNote) -> bool) -> Option<usize> {
        self.find_phase(Phase::NoteOn, &pred)
            .or_else(|| self.find_phase(Phase::Release, &pred))
            .or_else(|| self.find_phase(Phase::Kill, &pred))
            .map(|(idx, _)| idx)
    }

    fn take_phase(&mut self, phase: Phase) -> Option<(usize, PlayingNote)> {
        let (voice_idx, note) = self.find_phase_any(phase)?;
        self.voices[voice_idx] = None;
        Some((voice_idx, note))
    }

    fn assign_playing(&mut self, voice_idx: usize, note: Note) {
        let seq_idx = self.seq_idx;
        self.seq_idx = self.seq_idx.wrapping_add(1);
        self.voices[voice_idx] = Some(PlayingNote {
            note,
            seq_idx,
            phase: Phase::NoteOn,
        });
    }

    fn enter_phase(&mut self, voice_idx: usize, phase: Phase) {
        self.voices[voice_idx].as_mut().unwrap().phase = phase;
    }

    fn prev_note(&self, channel: u8, restored: bool) -> Option<PrevNote> {
        let channel = channel.min(MAX_MIDI_CHANNEL);
        let note = self.prev_notes[channel as usize]?;
        let is_same = |n: &Note| n.channel == channel && n.note == note;
        let voice_idx = self.find_active(|playing| is_same(&playing.note));
        let overlap = restored
            || self.waiting.iter().any(&is_same)
            || self.any_phase(Phase::NoteOn, |playing| is_same(&playing.note));

        Some(PrevNote {
            note,
            voice_idx: voice_idx.map(|idx| idx as u8),
            overlap,
        })
    }

    fn grab_and_reset(
        &mut self,
        replaced_idx: Option<usize>,
        note: Note,
        offset: usize,
        restored: bool,
        events: &mut VoiceEvents,
    ) {
        let prev_note = self.prev_note(note.channel, restored);

        let voice_idx = if let Some(voice_idx) = self.alloc_voice() {
            voice_idx
        } else if let Some((voice_idx, killing)) = self.take_phase(Phase::Kill) {
            self.terminate_killed(killing.note);
            voice_idx
        } else if let Some((voice_idx, releasing)) = self.take_phase(Phase::Release) {
            self.terminate.push(releasing.note);
            voice_idx
        } else if let Some((voice_idx, playing)) = self.take_phase(Phase::NoteOn) {
            self.waiting.push(playing.note);
            voice_idx
        } else {
            panic!("grab_and_reset(): Note processing error")
        };

        self.assign_playing(voice_idx, note);
        self.triggers[voice_idx] = Some(offset as u16);

        events.reset(voice_idx, replaced_idx, prev_note, note, offset);
    }

    fn legato(&mut self, voice_idx: usize, note: Note, offset: usize, events: &mut VoiceEvents) {
        self.assign_playing(voice_idx, note);
        events.update(voice_idx, note, offset);
    }

    fn terminate_killed(&mut self, note: Note) {
        let still_in_use = self
            .waiting
            .iter()
            .any(|waiting| waiting.is_same_instance(&note))
            || self.any_phase(Phase::NoteOn, |playing| {
                playing.note.is_same_instance(&note)
            });

        if !still_in_use {
            self.terminate.push(note);
        }
    }

    fn kill(&mut self, voice_idx: usize, offset: usize, events: &mut VoiceEvents) {
        self.enter_phase(voice_idx, Phase::Kill);
        events.kill(voice_idx, offset);
    }

    fn note_on_monophonic(
        &mut self,
        new_note: Note,
        offset: usize,
        restored: bool,
        events: &mut VoiceEvents,
    ) {
        // Kill playing note on same channel
        if let Some((voice_idx, playing)) = self.find_phase_channel(Phase::NoteOn, new_note.channel)
        {
            self.waiting.push(playing.note);

            if self.legato {
                self.legato(voice_idx, new_note, offset, events);
            } else {
                self.kill(voice_idx, offset, events);
                self.grab_and_reset(Some(voice_idx), new_note, offset, restored, events);
            }
        }
        // Kill releasing note on same channel
        else if let Some((voice_idx, _)) =
            self.find_phase_channel(Phase::Release, new_note.channel)
        {
            self.kill(voice_idx, offset, events);
            self.grab_and_reset(Some(voice_idx), new_note, offset, restored, events);
        } else {
            self.grab_and_reset(None, new_note, offset, restored, events);
        }
    }

    fn note_on_polyphonic(
        &mut self,
        new_note: Note,
        offset: usize,
        restored: bool,
        events: &mut VoiceEvents,
    ) {
        let mut replaced_voice_idx = None;

        // Kill the same releasing note so the retrigger replaces it. When disabled, the
        // new note layers on that release until a voice has to be stolen.
        if self.kill_same_note
            && let Some((voice_idx, _)) = self.find_phase_note(Phase::Release, &new_note)
        {
            self.kill(voice_idx, offset, events);
            replaced_voice_idx = Some(voice_idx);
        }

        // All available voices have been occupied; free the oldest, preferring a releasing voice.
        if self.count_phase(Phase::NoteOn) + self.count_phase(Phase::Release) >= self.num_voices {
            if let Some((voice_idx, _)) = self.find_phase_oldest(Phase::Release) {
                self.kill(voice_idx, offset, events);
            } else if let Some((voice_idx, playing)) = self.find_phase_oldest(Phase::NoteOn) {
                self.waiting.push(playing.note);
                self.kill(voice_idx, offset, events);
            }
        }

        self.grab_and_reset(replaced_voice_idx, new_note, offset, restored, events);
    }

    fn note_on_impl(
        &mut self,
        new_note: Note,
        offset: usize,
        restored: bool,
        events: &mut VoiceEvents,
    ) {
        let monophonic = self.num_voices == 1;

        // Ignore already pressed notes
        if self
            .waiting
            .iter()
            .any(|waiting| waiting.is_same_note(&new_note))
            || self.any_phase(Phase::NoteOn, |playing| {
                playing.note.is_same_note(&new_note)
            })
        {
            tracing::info!("Already pressed note came: {:?}", new_note);
            return;
        }

        if monophonic {
            self.note_on_monophonic(new_note, offset, restored, events);
        } else {
            self.note_on_polyphonic(new_note, offset, restored, events);
        }

        self.prev_notes[new_note.channel.min(MAX_MIDI_CHANNEL) as usize] = Some(new_note.note);
    }

    pub fn reset_triggers(&mut self) {
        self.triggers.fill(None);
    }

    pub fn handle_note_on(&mut self, note: Note, offset: usize, events: &mut VoiceEvents) {
        self.note_on_impl(note, offset, false, events);
    }

    pub fn handle_note_off(&mut self, note: Note, offset: usize, events: &mut VoiceEvents) {
        let monophonic = self.num_voices == 1;

        // Waiting note lifted - just remove it from the list
        if let Some(waiting_idx) = self
            .waiting
            .iter()
            .position(|waiting| waiting.is_same_note(&note))
        {
            let waiting = self.waiting.remove(waiting_idx);
            if !self.any_phase(Phase::Kill, |killing| {
                killing.note.is_same_instance(&waiting)
            }) {
                self.terminate.push(waiting);
            }
            return;
        }

        let Some((voice_idx, playing)) = self.find_phase_note(Phase::NoteOn, &note) else {
            tracing::info!("Unknown note lifted: {:?}", note);
            return;
        };

        if monophonic
            && self.legato
            && let Some(waiting_idx) = self
                .waiting
                .iter()
                .rposition(|waiting| waiting.channel == note.channel)
        {
            let waiting_note = self.waiting.remove(waiting_idx);

            self.terminate.push(playing.note);
            self.prev_notes[note.channel.min(MAX_MIDI_CHANNEL) as usize] = Some(waiting_note.note);
            self.legato(voice_idx, waiting_note, offset, events);
            return;
        }

        self.enter_phase(voice_idx, Phase::Release);
        events.release(voice_idx, note.velocity, offset);

        // In mono mode `waiting` is a per-channel held-notes stack; only restore
        // notes from the released note's channel.
        let waiting_note = if monophonic {
            self.waiting
                .iter()
                .rposition(|waiting| waiting.channel == note.channel)
                .map(|idx| self.waiting.remove(idx))
        } else {
            self.waiting.pop()
        };

        if let Some(waiting_note) = waiting_note {
            self.note_on_impl(waiting_note, offset, true, events);
        }
    }

    pub fn handle_choke(&mut self, note: Note) {
        let mut found = false;

        if let Some((voice_idx, _)) = self.find_phase_note(Phase::NoteOn, &note) {
            self.voices[voice_idx] = None;
            found = true;
        } else if let Some((voice_idx, _)) = self.find_phase_note(Phase::Release, &note) {
            self.voices[voice_idx] = None;
            found = true;
        } else if let Some((voice_idx, _)) = self.find_phase_note(Phase::Kill, &note) {
            self.voices[voice_idx] = None;
            found = true;
        }

        if let Some(waiting_idx) = self
            .waiting
            .iter()
            .position(|waiting| waiting.is_same_note(&note))
        {
            self.waiting.remove(waiting_idx);
            found = true;
        }

        if found {
            self.terminate.push(note);
        }
    }

    pub fn choke_all_voices(&mut self) {
        for idx in 0..MAX_VOICES {
            let Some(voice) = self.voices[idx].take() else {
                continue;
            };

            match voice.phase {
                Phase::NoteOn | Phase::Release => {
                    self.terminate.push(voice.note);
                }
                Phase::Kill => {
                    if !self
                        .waiting
                        .iter()
                        .any(|waiting| waiting.is_same_instance(&voice.note))
                    {
                        self.terminate.push(voice.note);
                    }
                }
            }
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
            .find_phase_note(Phase::NoteOn, &note)
            .or_else(|| self.find_phase_note(Phase::Release, &note))
            .map(|(voice_idx, _)| voice_idx);

        voice_idx.map(|voice_idx| ExpressionEvent {
            voice_idx,
            expression,
            offset,
            value,
        })
    }

    pub fn voice_idx_for_host_id(&self, host_id: i32) -> Option<usize> {
        self.find_active(|note| note.note.host_id == Some(host_id))
    }

    pub fn set_num_voices(&mut self, num_voices: usize) {
        self.num_voices = num_voices.clamp(1, MAX_AVAILABLE_VOICES);
    }

    pub fn set_legato(&mut self, legato: bool) {
        self.legato = legato;
    }

    pub fn set_kill_same_note(&mut self, kill_same_note: bool) {
        self.kill_same_note = kill_same_note;
    }

    pub fn get_metrics(&self) -> VoicesHandlerMetrics {
        VoicesHandlerMetrics {
            num_voices: self.num_voices,
            legato: self.legato,
            kill_same_note: self.kill_same_note,
            waiting: self.waiting.len(),
            playing: self.count_phase(Phase::NoteOn),
            releasing: self.count_phase(Phase::Release),
            killing: self.count_phase(Phase::Kill),
        }
    }

    pub fn get_decaying_voices(&self, decaying_voices: &mut DecayingVoices) {
        for (idx, slot) in self.voices.iter().enumerate() {
            let Some(note) = slot else {
                continue;
            };
            if note.phase == Phase::Release || note.phase == Phase::Kill {
                decaying_voices.push(DecayingVoice::new(idx as u8));
            }
        }
    }

    pub fn update_decaying_voices(
        &mut self,
        decaying_voices: &[DecayingVoice],
        terminated: &mut Vec<Note>,
    ) {
        for decaying in decaying_voices.iter().filter(|decaying| decaying.is_done()) {
            let idx = decaying.index();
            let Some(voice) = self.voices[idx] else {
                continue;
            };

            match voice.phase {
                Phase::Release => {
                    self.voices[idx] = None;
                    self.terminate.push(voice.note);
                }
                Phase::Kill => {
                    self.voices[idx] = None;
                    self.terminate_killed(voice.note);
                }
                Phase::NoteOn => {}
            }
        }

        terminated.append(&mut self.terminate);
    }

    pub fn get_playing_voices(&mut self, playing_voices: &mut PlayingVoices) -> Option<usize> {
        let mut last_voice_idx = None;
        let mut last_seq = 0;

        for (idx, slot) in self.voices.iter().enumerate() {
            let Some(note) = slot else {
                continue;
            };

            if last_voice_idx.is_none() || note.seq_idx >= last_seq {
                last_seq = note.seq_idx;
                last_voice_idx = Some(idx);
            }

            playing_voices.push(PlayingVoice::new(idx as u8, note.note.note));
        }

        for playing in playing_voices.iter_mut() {
            playing.triggered = self.triggers[playing.voice_idx()];
        }

        last_voice_idx
    }
}

#[cfg(test)]
mod tests;
