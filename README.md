## Overview

Additizer is a modular synthesizer plugin. It consists of a wavetable-like oscillator
and a set of modules that process waveforms in the frequency domain.
Every slider in the UI is stereo, and each channel can be adjusted by dragging with the right mouse button.

## Modules

- `Harmonic Editor`: Allows you to set each of the 1024 harmonics manually, apply a biquad filter statically, or set the gain for a group of harmonics
  selected by range and an n-th-element formula.
- `Spectral Noise`: Generates white, pink, or brown noise harmonics.
- `Spectral Filter`: Applies a biquad filter to the frequency bins (lowpass, highpass, bandpass, bandstop, and peaking).
  Has a 4th-order option (multiply by the filter frequency response twice) and a linear-phase mode (multiply by the magnitude response).
- `Spectral EQ`: Chains up to 16 spectral filters.
- `Spectral Mixer`: Mixes multiple spectral inputs with per-input level/gain and output volume control.
- `Spectral Blend`: Crossfades between two spectrums with a blend control.
- `Band Select`: Passes a half-open range of harmonics.
- `Oscillator`: Takes a spectral input, performs an inverse FFT, and then behaves like a wavetable oscillator.
  Supports up to 16 unison voices, each of which is stereo. Manual unison sets each voice's phase and gain with a stereo slider. Flat and Convex unison shape gains as a center voice against the detuned voices and spread them across the channels.
  Controls that can be modulated: gain, pitch, frequency (through-zero FM), phase, detune, detune focus (pitch distribution),
  unison phases and unison levels blend.
- `Envelope`: AHDSR envelope generator to control both spectral and audio modules.
- `Segments Envelope`: Breakpoint envelope. A release point sustains while the key is held, then continues from the current level. A loop point repeats to the end, or up to the release point while the key is held. The length is a number of seconds or note values.
- `LFO`: Low-frequency oscillator with a drawable breakpoint waveform.
- `Mixer`: Mixes multiple audio inputs with per-input level/gain and output volume control.
- `Waveshaper`: Wave shaping distortion (hard clip or sigmoid) with drive and clipping level.
- `SVF`: Time-domain state-variable filter (Cytomic / Andrew Simper form): lowpass and highpass at 12/18/24 dB,
  bandpass at 6/12/18/24 dB per octave (18 dB lowpass and highpass cascade a one-pole section after the SVF;
  steeper bandpass cascades extra SVF stages).
  Modulatable cutoff (with keytrack), resonance, and drive (soft saturation ahead of the filter).
- `Amplifier`: Simple gain modulation for input signal.
- `External Parameter`: Exposes host/plugin parameters as modulation sources with smoothing.
- `Expressions`: Uses MPE as modulation sources.

## Build

To build the standalone app and the CLAP plugin in `./target/bundled`:

```shell
cargo nice-plug bundle additizer --release
```

## Run

To run the plugin in standalone mode, specify your MIDI keyboard by name.

```shell
cargo run --release -- --midi-input "Keystation Mini 32 MK3"
```
