## Overview
Additizer is a modular synthesizer plugin. It consists of a wavetable-like oscillator inspired by Matt Tytel's Vital
and a set of modules that process waveforms in the frequency domain.
Every slider in the UI is stereo, and each channel can be adjusted by dragging with the right mouse button.

## Synth Engine Modules
- `Harmonic Editor`: Allows you to set each of the 1024 harmonics manually, apply a biquad filter statically, or set the gain for a group of harmonics
    selected by range and an n-th-element formula.
- `Spectral Filter`: Applies a biquad filter to the frequency bins (lowpass, highpass, bandpass, bandstop, and peaking).
    Has a 4th-order option (multiply by the filter frequency response twice) and a linear-phase mode (multiply by the magnitude response).
- `Spectral Mixer`: Mixes multiple spectral inputs with per-input level/gain and output volume control.
- `Spectral Blend`: Crossfades between two spectrums with a blend control.
- `Oscillator`: Takes a spectral input, performs an inverse FFT, and then behaves like a wavetable oscillator.
    Supports up to 16 unison voices, each of which is stereo. The phase and gain of each unison voice can be controlled via a stereo slider.
    Controls that can be modulated: gain, pitch, frequency (through-zero FM), phase, detune, detune power (pitch distribution),
    unison phases and unison gains blend.
- `Envelope`: AHDSR envelope generator to control both spectral and audio modules.
- `LFO`: Low-frequency oscillator (triangle/square/sine) with skew and bipolar modes.
- `Mixer`: Mixes multiple audio inputs with per-input level/gain and output volume control.
- `Waveshaper`: Wave shaping distortion (hard clip or sigmoid) with drive and clipping level.
- `Amplifier`: Simple gain modulation for input signal.
- `External Parameter`: Exposes host/plugin parameters as modulation sources with smoothing or sample-and-hold.
- `Expressions`: Uses MPE as modulation sources.

## Build
To build the standalone app and the CLAP plugin in `./target/bundled`:
```shell
cargo nih-plug bundle additizer --release
```

## Run
To run the plugin in standalone mode, specify your MIDI keyboard by name.
```shell
cargo run --release -- --midi-input "Keystation Mini 32 MK3"
```
