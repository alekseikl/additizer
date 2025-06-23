const PHASE_PERIOD_MASK: u64 = u32::MAX as u64;
const PHASE_PERIOD: u64 = u32::MAX as u64 + 1;
const PHASE_PERIOD_F64: f64 = PHASE_PERIOD as f64;

#[derive(Debug, Clone, Copy)]
pub struct Phase(u64);

impl Phase {
    pub fn new(initial_phase: f32) -> Self {
        Self((PHASE_PERIOD as f64 * initial_phase as f64) as u64 & PHASE_PERIOD_MASK)
    }

    #[inline]
    pub fn for_harmonic(&self, harmonic: usize, table_size: usize) -> usize {
        ((self.0.wrapping_mul(harmonic as u64) & PHASE_PERIOD_MASK) * table_size as u64
            / PHASE_PERIOD) as usize
    }

    #[inline]
    pub fn for_subharmonic(&self, subharmonic: usize, table_size: usize) -> usize {
        ((self.0.wrapping_div(subharmonic as u64) & PHASE_PERIOD_MASK) * table_size as u64
            / PHASE_PERIOD) as usize
    }

    #[inline]
    pub fn advance(&mut self, sample_rate: f32, frequency: f32) {
        self.0 = self
            .0
            .wrapping_add((PHASE_PERIOD_F64 * (frequency / sample_rate) as f64) as u64);
    }
}
