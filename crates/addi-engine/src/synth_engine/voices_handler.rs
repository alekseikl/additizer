use smallvec::SmallVec;

use crate::synth_engine::{
    Expression, Smp,
    buffer::{MonoVoicesLayout, new_mono_voices_layout},
    note_to_pitch,
    routing::{ExpressionEvent, PrevNote, VoiceEvent},
    voices::{AVAILABLE_VOICES, MAX_VOICES, VOICE_BLOCK_LEN},
};

pub const BAND_LIMIT_FREQUENCY: Smp = 24_000.0;
const NUM_MIDI_CHANNELS: usize = 16;
const MAX_MIDI_CHANNEL: u8 = NUM_MIDI_CHANNELS as u8 - 1;
const MAX_NOTE_EVENTS: usize = MAX_VOICES * 4;

const _: () = assert!(MAX_VOICES <= u8::MAX as usize);

#[derive(Debug, Clone, Copy)]
pub struct Note {
    pub channel: u8,
    pub note: u8,
    pub velocity: Smp,
    pub host_id: Option<i32>,
}

impl Note {
    fn matches(&self, event: &Self) -> bool {
        self.channel == event.channel && self.note == event.note
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    NoteOn,
    Release,
    Kill,
}

#[derive(Clone, Copy)]
struct Slot {
    note: Note,
    seq_idx: u32,
    phase: Phase,
    owns_host_id: bool,
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

pub struct VoicesConfig {
    pub num_voices: usize,
    pub legato: bool,
    pub kill_same_note: bool,
}

pub struct VoicesTelemetry {
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
    voices: [Option<Slot>; MAX_VOICES],
    terminate: Vec<Note>,
    prev_notes: [Option<u8>; NUM_MIDI_CHANNELS],
    triggers: MonoVoicesLayout<Option<u16>>,
    seq_idx: u32,
    events: Vec<VoiceEvent>,
}

impl VoicesHandler {
    pub fn new(num_voices: usize, legato: bool, kill_same_note: bool) -> Self {
        Self {
            num_voices: num_voices.clamp(1, AVAILABLE_VOICES),
            legato,
            kill_same_note,
            waiting: Vec::with_capacity(64),
            voices: [None; MAX_VOICES],
            terminate: Vec::with_capacity(64),
            prev_notes: [None; NUM_MIDI_CHANNELS],
            triggers: new_mono_voices_layout(),
            seq_idx: 0,
            events: Vec::with_capacity(MAX_NOTE_EVENTS),
        }
    }

    pub fn get_events(&self) -> &[VoiceEvent] {
        &self.events
    }

    pub fn clear_events(&mut self) {
        self.events.clear();
    }

    fn push_reset(
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
            pitch: note_to_pitch(note.note as f32),
            velocity: note.velocity,
            offset,
        });
    }

    fn push_update(&mut self, voice_idx: usize, note: Note, offset: usize) {
        self.events.push(VoiceEvent::Update {
            voice_idx,
            pitch: note_to_pitch(note.note as f32),
            velocity: note.velocity,
            offset,
        });
    }

    fn push_release(&mut self, voice_idx: usize, velocity: Smp, offset: usize) {
        self.events.push(VoiceEvent::Release {
            voice_idx,
            velocity,
            offset,
        });
    }

    fn push_kill(&mut self, voice_idx: usize, offset: usize) {
        self.events.push(VoiceEvent::Kill { voice_idx, offset });
    }

    /// Prefer a free slot in a block that already holds a voice, so its lanes stay occupied.
    /// Otherwise take the first slot of the first fully free block.
    fn alloc_voice(&self) -> Option<usize> {
        let mut free_slot = None;
        let (blocks, _) = self.voices.as_chunks::<VOICE_BLOCK_LEN>();

        for (block_idx, block) in blocks.iter().enumerate() {
            let base = block_idx * VOICE_BLOCK_LEN;
            let mut occupied = 0;
            let mut first_free = None;

            for (offset, slot) in block.iter().enumerate() {
                if slot.is_some() {
                    occupied += 1;
                } else if first_free.is_none() {
                    first_free = Some(base + offset);
                }
            }

            if occupied > 0 && occupied < VOICE_BLOCK_LEN {
                return first_free;
            }

            if occupied == 0 && free_slot.is_none() {
                free_slot = Some(base);
            }
        }

        free_slot
    }

    fn occupied_voices(&self) -> usize {
        self.voices
            .iter()
            .flatten()
            .filter(|slot| slot.phase != Phase::Kill)
            .count()
    }

    fn find_slot(&self, phase: Phase, pred: impl Fn(&Slot) -> bool) -> Option<(usize, Slot)> {
        self.voices.iter().enumerate().find_map(|(idx, slot)| {
            let note = *slot.as_ref()?;
            (note.phase == phase && pred(&note)).then_some((idx, note))
        })
    }

    fn has_slot_with_note(&self, phase: Phase, note: &Note) -> bool {
        self.voices
            .iter()
            .flatten()
            .any(|slot| slot.phase == phase && slot.note.matches(note))
    }

    fn slot_with_note(&self, phase: Phase, note: &Note) -> Option<(usize, Slot)> {
        self.find_slot(phase, |playing| playing.note.matches(note))
    }

    fn slot_with_channel(&self, phase: Phase, channel: u8) -> Option<(usize, Slot)> {
        self.find_slot(phase, |playing| playing.note.channel == channel)
    }

    fn oldest_slot(&self, phase: Phase) -> Option<(usize, Slot)> {
        self.voices
            .iter()
            .enumerate()
            .filter_map(|(idx, slot)| {
                let note = *slot.as_ref()?;
                (note.phase == phase).then_some((idx, note))
            })
            .min_by_key(|(_, note)| note.seq_idx)
    }

    fn take_slot(&mut self, phase: Phase) -> Option<(usize, Slot)> {
        let (voice_idx, note) = self.find_slot(phase, |_| true)?;
        self.voices[voice_idx] = None;
        Some((voice_idx, note))
    }

    fn assign_slot(&mut self, voice_idx: usize, note: Note) {
        let seq_idx = self.seq_idx;
        self.seq_idx = self.seq_idx.wrapping_add(1);
        self.voices[voice_idx] = Some(Slot {
            note,
            seq_idx,
            phase: Phase::NoteOn,
            owns_host_id: true,
        });
    }

    fn enter_phase(&mut self, voice_idx: usize, phase: Phase) {
        self.voices[voice_idx]
            .as_mut()
            .expect("voice slot is in place")
            .phase = phase;
    }

    fn prev_note(&self, channel: u8, overlapped: Option<Note>) -> Option<PrevNote> {
        let channel = channel.min(MAX_MIDI_CHANNEL);
        let note = overlapped
            .map(|note| note.note)
            .or_else(|| self.prev_notes[channel as usize])?;
        let voice_idx = self
            .voices
            .iter()
            .enumerate()
            .filter_map(|(idx, slot)| {
                let playing = *slot.as_ref()?;
                (playing.note.channel == channel && playing.note.note == note)
                    .then_some((idx, playing.seq_idx))
            })
            .max_by_key(|(_, seq_idx)| *seq_idx)
            .map(|(idx, _)| idx as u8);

        Some(PrevNote {
            note,
            voice_idx,
            overlap: overlapped.is_some(),
        })
    }

    fn grab(
        &mut self,
        replaced_idx: Option<usize>,
        note: Note,
        offset: usize,
        overlapped: Option<Note>,
    ) {
        let prev_note = self.prev_note(note.channel, overlapped);

        let voice_idx = if let Some(voice_idx) = self.alloc_voice() {
            voice_idx
        } else if let Some((voice_idx, killing)) = self.take_slot(Phase::Kill) {
            self.terminate(&killing);
            voice_idx
        } else if let Some((voice_idx, releasing)) = self.take_slot(Phase::Release) {
            self.terminate(&releasing);
            voice_idx
        } else if let Some((voice_idx, playing)) = self.take_slot(Phase::NoteOn) {
            self.waiting.push(playing.note);
            voice_idx
        } else {
            panic!("grab(): Note processing error")
        };

        self.assign_slot(voice_idx, note);
        self.triggers[voice_idx] = Some(offset as u16);

        self.push_reset(voice_idx, replaced_idx, prev_note, note, offset);
    }

    fn legato(&mut self, voice_idx: usize, note: Note, offset: usize) {
        self.assign_slot(voice_idx, note);
        self.push_update(voice_idx, note, offset);
    }

    fn terminate(&mut self, slot: &Slot) {
        if slot.owns_host_id {
            self.terminate.push(slot.note);
        }
    }

    fn kill(&mut self, voice_idx: usize, offset: usize) {
        self.enter_phase(voice_idx, Phase::Kill);
        self.push_kill(voice_idx, offset);
    }

    fn kill_and_disown(&mut self, voice_idx: usize, offset: usize) {
        self.voices[voice_idx]
            .as_mut()
            .expect("voice slot is in place")
            .owns_host_id = false;
        self.kill(voice_idx, offset);
    }

    fn note_on_monophonic(&mut self, new_note: Note, offset: usize, restored_after: Option<Slot>) {
        // Kill playing note on same channel
        if let Some((voice_idx, playing)) = self.slot_with_channel(Phase::NoteOn, new_note.channel)
        {
            self.waiting.push(playing.note);

            if self.legato {
                self.legato(voice_idx, new_note, offset);
            } else {
                self.kill_and_disown(voice_idx, offset);
                self.grab(Some(voice_idx), new_note, offset, Some(playing.note));
            }
        }
        // Kill releasing note on same channel
        else if let Some((voice_idx, _)) =
            self.slot_with_channel(Phase::Release, new_note.channel)
        {
            self.kill(voice_idx, offset);
            let overlapped = restored_after.map(|slot| slot.note);
            self.grab(Some(voice_idx), new_note, offset, overlapped);
        } else {
            self.grab(None, new_note, offset, None);
        }
    }

    fn note_on_polyphonic(&mut self, new_note: Note, offset: usize, restored_after: Option<Slot>) {
        let mut replaced_idx = None;

        // Kill the same releasing note so the retrigger replaces it. When disabled, the
        // new note layers on that release until a voice has to be stolen.
        if self.kill_same_note
            && let Some((voice_idx, _)) = self.slot_with_note(Phase::Release, &new_note)
        {
            self.kill(voice_idx, offset);
            replaced_idx = Some(voice_idx);
        }

        // All available voices have been occupied; free the oldest, preferring a releasing voice.
        let mut stolen = None;
        if self.occupied_voices() >= self.num_voices {
            if let Some((voice_idx, _)) = self.oldest_slot(Phase::Release) {
                self.kill(voice_idx, offset);
            } else if let Some((voice_idx, playing)) = self.oldest_slot(Phase::NoteOn) {
                stolen = Some(playing.note);
                self.waiting.push(playing.note);
                self.kill_and_disown(voice_idx, offset);
            }
        }

        let channel = new_note.channel;
        let overlapped = if let Some(released) = restored_after {
            Some(released.note)
        } else {
            self.voices
                .iter()
                .flatten()
                .filter(|slot| slot.note.channel == channel && slot.phase == Phase::NoteOn)
                .max_by_key(|slot| slot.seq_idx)
                .map(|slot| slot.note)
                .or_else(|| stolen.filter(|note| note.channel == channel))
        };
        self.grab(replaced_idx, new_note, offset, overlapped);
    }

    fn note_on_impl(
        &mut self,
        new_note: Note,
        offset: usize,
        restored_after: Option<Slot>,
    ) -> bool {
        let monophonic = self.num_voices == 1;

        if self
            .waiting
            .iter()
            .any(|waiting| waiting.matches(&new_note))
            || self.has_slot_with_note(Phase::NoteOn, &new_note)
        {
            tracing::info!("Already pressed note came: {:?}", new_note);
            return false;
        }

        if monophonic {
            self.note_on_monophonic(new_note, offset, restored_after);
        } else {
            self.note_on_polyphonic(new_note, offset, restored_after);
        }

        self.prev_notes[new_note.channel.min(MAX_MIDI_CHANNEL) as usize] = Some(new_note.note);
        true
    }

    pub fn reset_triggers(&mut self) {
        self.triggers.fill(None);
    }

    pub fn handle_note_on(&mut self, note: Note, offset: usize) {
        self.note_on_impl(note, offset, None);
    }

    /// Releases `note`. Returns a note that started playing again, so pending modulation for its
    /// host id can be applied.
    pub fn handle_note_off(&mut self, note: Note, offset: usize) -> Option<Note> {
        let monophonic = self.num_voices == 1;

        // A parked note owns its host id, so lifting it terminates that id.
        if let Some(waiting_idx) = self
            .waiting
            .iter()
            .position(|waiting| waiting.matches(&note))
        {
            self.terminate.push(self.waiting.remove(waiting_idx));
            return None;
        }

        let Some((voice_idx, playing)) = self.slot_with_note(Phase::NoteOn, &note) else {
            tracing::info!("Unknown note lifted: {:?}", note);
            return None;
        };

        let channel = playing.note.channel;

        if monophonic
            && self.legato
            && let Some(waiting_idx) = self
                .waiting
                .iter()
                .rposition(|waiting| waiting.channel == channel)
        {
            let waiting_note = self.waiting.remove(waiting_idx);

            self.terminate(&playing);
            self.prev_notes[channel.min(MAX_MIDI_CHANNEL) as usize] = Some(waiting_note.note);
            self.legato(voice_idx, waiting_note, offset);
            return Some(waiting_note);
        }

        self.enter_phase(voice_idx, Phase::Release);
        self.push_release(voice_idx, note.velocity, offset);

        // In mono mode `waiting` is a per-channel held-notes stack; only restore
        // notes from the released note's channel.
        let waiting_note = if monophonic {
            self.waiting
                .iter()
                .rposition(|waiting| waiting.channel == channel)
                .map(|idx| self.waiting.remove(idx))
        } else {
            self.waiting.pop()
        };

        if let Some(waiting_note) = waiting_note
            && self.note_on_impl(waiting_note, offset, self.voices[voice_idx])
        {
            return Some(waiting_note);
        }

        None
    }

    pub fn handle_choke(&mut self, note: Note) {
        let voices: SmallVec<[usize; MAX_VOICES]> = self
            .voices
            .iter()
            .enumerate()
            .filter_map(|(idx, slot)| {
                slot.as_ref()
                    .filter(|playing| playing.note.matches(&note))
                    .map(|_| idx)
            })
            .collect();

        for voice_idx in voices {
            if let Some(playing) = self.voices[voice_idx].take() {
                self.terminate(&playing);
            }
        }

        for held in self.waiting.extract_if(.., |held| held.matches(&note)) {
            self.terminate.push(held);
        }
    }

    pub fn choke_all_voices(&mut self) {
        for idx in 0..MAX_VOICES {
            let Some(voice) = self.voices[idx].take() else {
                continue;
            };

            self.terminate(&voice);
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
        value: Smp,
    ) -> Option<ExpressionEvent> {
        let (voice_idx, _) = self.slot_with_note(Phase::NoteOn, &note)?;

        Some(ExpressionEvent {
            voice_idx,
            expression,
            offset,
            value,
        })
    }

    pub fn voice_idx_for_host_id(&self, host_id: i32) -> SmallVec<[usize; 2]> {
        self.voices
            .iter()
            .enumerate()
            .filter_map(|(voice_idx, slot)| {
                let playing = slot.as_ref()?;
                (playing.note.host_id == Some(host_id)).then_some(voice_idx)
            })
            .take(2)
            .collect()
    }

    pub fn set_num_voices(&mut self, num_voices: usize) {
        self.num_voices = num_voices.clamp(1, AVAILABLE_VOICES);

        let excess = self.occupied_voices().saturating_sub(self.num_voices);

        if excess == 0 {
            return;
        }

        let mut excess_voices = SmallVec::<[usize; MAX_VOICES]>::new();

        for (idx, slot) in self.voices.iter().enumerate() {
            if excess_voices.len() == excess {
                break;
            }

            if let Some(slot) = slot
                && slot.phase != Phase::Kill
            {
                excess_voices.push(idx);
            }
        }

        for voice_idx in excess_voices {
            self.kill(voice_idx, 0);
        }
    }

    pub fn set_legato(&mut self, legato: bool) {
        self.legato = legato;
    }

    pub fn set_kill_same_note(&mut self, kill_same_note: bool) {
        self.kill_same_note = kill_same_note;
    }

    pub fn get_config(&self) -> VoicesConfig {
        VoicesConfig {
            num_voices: self.num_voices,
            legato: self.legato,
            kill_same_note: self.kill_same_note,
        }
    }

    pub fn get_telemetry(&self) -> VoicesTelemetry {
        let mut playing = 0;
        let mut releasing = 0;
        let mut killing = 0;

        for slot in self.voices.iter().flatten() {
            match slot.phase {
                Phase::NoteOn => playing += 1,
                Phase::Release => releasing += 1,
                Phase::Kill => killing += 1,
            }
        }

        VoicesTelemetry {
            waiting: self.waiting.len(),
            playing,
            releasing,
            killing,
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
                Phase::Release | Phase::Kill => {
                    self.voices[idx] = None;
                    self.terminate(&voice);
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
