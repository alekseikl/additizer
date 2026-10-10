//! Sources that fill an arena slot for a benchmark.
//!
//! Each one allocates with `OutputsArena::allocate_slot` and writes through the
//! router. Benchmarks run them before the timer.

mod audio;
mod control;
mod fill;
mod spectral;

#[allow(unused_imports)]
pub use audio::Audio;
pub use control::Control;
pub use spectral::Spectral;
