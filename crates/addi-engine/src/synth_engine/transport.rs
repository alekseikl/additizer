use crate::synth_engine::Smp;

/// Tempo and song position for one call to [`SynthEngine::process`](super::SynthEngine::process).
///
/// The engine borrows the implementor for that call and does not store it, so the concrete
/// transport can live with the host.
pub trait HostTransport {
    /// Beats per minute.
    fn tempo(&self) -> f64;
    fn pos_seconds(&self) -> f64;
    fn pos_beats(&self) -> f64;
    /// Host samples from the song position to sample 0 of the block.
    /// Negative when the position falls inside the block.
    fn origin(&self) -> isize;
}

/// Host timeline for the block currently being processed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostTimeline {
    seconds_per_sample: f64,
    tempo: f64,
    pos_seconds: f64,
    pos_beats: f64,
    /// Internal samples from the song position to sample 0 of this block.
    origin: isize,
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
            origin: 0,
        }
    }

    pub(crate) fn from_transport(
        sample_rate: f64,
        transport: &impl HostTransport,
        oversampling: usize,
    ) -> Self {
        Self {
            seconds_per_sample: sample_rate.recip(),
            tempo: transport.tempo(),
            pos_seconds: transport.pos_seconds(),
            pos_beats: transport.pos_beats(),
            origin: transport.origin() * oversampling as isize,
        }
    }

    /// Beats per minute.
    pub fn tempo(&self) -> Smp {
        self.tempo as Smp
    }

    /// Seconds from the transport position to `sample_offset` into the current block.
    pub fn seconds_offset(&self, sample_offset: usize) -> f64 {
        (self.origin + sample_offset as isize) as f64 * self.seconds_per_sample
    }

    /// Quarter notes from the transport position to `sample_offset` into the current block.
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

#[cfg(test)]
pub(crate) mod tests;
