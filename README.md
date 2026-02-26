## Overview
Additizer is a modular synthesizer plugin. It consists of a wavetable-like oscillator inspired by Matt Tytel's Vital 
and a set of modules that process waveforms in frequency domain.
Every slider in UI is stereo, individual channel can be changed by dragging with right mouse button.

## Synth Engine Modules
- `Harmonic Editor`: Edits the harmonic spectrum by setting individual or ranged partial gains.
- `Spectral Filter`: Spectral-domain filter (LP/HP/BP/BS/peaking) with optional linear-phase and 4th-order responses.
- `Spectral Mixer`: Mixes multiple spectral inputs with selectable mix/volume types and output gain.
- `Spectral Blend`: Crossfades between two spectra with a blend control.
- `Oscillator`: Spectral oscillator with unison, detune, pitch shift, phase and frequency shift (through-zero FM).
- `Envelope`: ADSR-style envelope generator with selectable curve shapes per stage.
- `LFO`: Low-frequency oscillator (triangle/square/sine) for modulation, with skew and bipolar modes.
- `Mixer`: Mixes multiple audio inputs with per-input level/gain and output volume control.
- `Waveshaper`: Wave shaping distortion (hard clip or sigmoid) with drive and clipping level.
- `Amplifier`: Simple gain stage for audio with gain modulation input.
- `External Parameter`: Exposes host/plugin float parameters as modulation sources with smoothing or sample-and-hold.
- `Modulation Filter`: Low-pass filter to smooth modulation signals.

## Build
```shell
cargo nih-plug bundle additizer --release
```
