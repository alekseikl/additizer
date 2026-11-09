use crate::synth_engine::Smp;

/// Tempo and song position reported by the host for one buffer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransportState {
    /// Beats per minute.
    pub tempo: f64,
    pub pos_seconds: f64,
    pub pos_beats: f64,
}

/// Host timeline for the block currently being processed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostTimeline {
    seconds_per_sample: f64,
    tempo: f64,
    pos_seconds: f64,
    pos_beats: f64,
    block_start: usize,
}

impl Default for HostTimeline {
    fn default() -> Self {
        Self::new(48_000.0, 120.0, 0.0, 0.0)
    }
}

impl HostTimeline {
    pub fn new(sample_rate: f64, tempo: f64, pos_seconds: f64, pos_beats: f64) -> Self {
        Self {
            seconds_per_sample: sample_rate.recip(),
            tempo,
            pos_seconds,
            pos_beats,
            block_start: 0,
        }
    }

    pub(crate) fn from_transport(
        sample_rate: f64,
        transport: TransportState,
        block_start: usize,
    ) -> Self {
        Self {
            seconds_per_sample: sample_rate.recip(),
            tempo: transport.tempo,
            pos_seconds: transport.pos_seconds,
            pos_beats: transport.pos_beats,
            block_start,
        }
    }

    /// Beats per minute.
    pub fn tempo(&self) -> Smp {
        self.tempo as Smp
    }

    /// Seconds from the start of the buffer to `sample_offset` into the current block.
    pub fn seconds_offset(&self, sample_offset: usize) -> f64 {
        (self.block_start + sample_offset) as f64 * self.seconds_per_sample
    }

    /// Quarter notes from the start of the buffer to `sample_offset` into the current block.
    pub fn beats_offset(&self, sample_offset: usize) -> f64 {
        self.seconds_offset(sample_offset) * self.tempo * 60.0_f64.recip()
    }

    /// Seconds at `sample_offset` into the current block.
    pub fn seconds_at(&self, sample_offset: usize) -> f64 {
        self.pos_seconds + self.seconds_offset(sample_offset)
    }

    /// Quarter notes at `sample_offset` into the current block.
    pub fn beats_at(&self, sample_offset: usize) -> f64 {
        self.pos_beats + self.beats_offset(sample_offset)
    }
}
