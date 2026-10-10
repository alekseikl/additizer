//! Sources that fill an arena slot for a benchmark.
//!
//! Each one allocates with `OutputsArena::allocate_slot` and writes through the
//! router. Benchmarks run them before the timer.

#[allow(dead_code)]
mod audio;
#[allow(dead_code)]
mod control;
mod fill;
#[allow(dead_code)]
mod spectral;

#[allow(unused_imports)]
pub use audio::Audio;
#[allow(unused_imports)]
pub use control::Control;
#[allow(unused_imports)]
pub use spectral::Spectral;
