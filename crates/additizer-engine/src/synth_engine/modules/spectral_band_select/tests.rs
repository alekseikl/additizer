use super::*;
use crate::synth_engine::{C4_PITCH, pitch_to_freq};

fn quantized_bounds(freq_from: Sample, freq_to: Sample, pitch: Sample) -> (u16, u16) {
    let fundamental = pitch_to_freq(pitch).max(1.0);

    (
        <SpectralBandSelect>::quantize_harmonic(freq_from / fundamental),
        <SpectralBandSelect>::quantize_harmonic(freq_to / fundamental),
    )
}

#[test]
fn frequency_range_maps_through_pitch_like_the_spectral_filter() {
    let f0 = pitch_to_freq(C4_PITCH);

    assert_eq!(quantized_bounds(3.0 * f0, 6.0 * f0, C4_PITCH), (3, 6));
}

#[test]
fn frequency_range_follows_an_octave_of_pitch() {
    let f0 = pitch_to_freq(C4_PITCH);
    let (lo, hi) = quantized_bounds(2.0 * f0, 4.0 * f0, C4_PITCH + 1.0);

    assert_eq!(lo, 1);
    assert_eq!(hi, 2);
}

#[test]
fn frequency_gap_between_harmonics_selects_nothing() {
    let f0 = pitch_to_freq(C4_PITCH);
    let (lo, hi) = quantized_bounds(2.2 * f0, 2.8 * f0, C4_PITCH);

    assert!(hi <= lo);
}

#[test]
fn config_clamps_harmonic_and_frequency_limits() {
    let module = <SpectralBandSelect>::from_config(&SpectralBandSelectConfig {
        id: 3,
        mode: BandSelectMode::Frequency,
        harmonic_from: 0,
        harmonic_to: u16::MAX,
        freq_from: 1.0,
        freq_to: 100_000.0,
    });
    let config = module.get_config();

    assert_eq!(config.harmonic_from, MIN_HARMONIC);
    assert_eq!(config.harmonic_to, MAX_HARMONIC_END);
    assert_eq!(config.freq_from, MIN_BAND_HZ);
    assert_eq!(config.freq_to, MAX_BAND_HZ);
}
