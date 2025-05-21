pub mod adsr;
pub mod fade_out;
pub trait Envelope {
    fn value(&self) -> f32;
    fn is_done(&self) -> bool;
    fn advance(&mut self, sample_rate: f32);
    fn release(&mut self);
}
