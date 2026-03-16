## Overview
Additizer is a modular synthesizer plugin. It consists of a wavetable-like oscillator inspired by Matt Tytel's Vital 
and a set of modules that process waveforms in frequency domain.
Every slider in UI is stereo, individual channel can be changed by dragging with right mouse button.

## Synth Engine Modules
- `Harmonic Editor`: Allows to set each of 1024 harmonics manually, apply biquad filter statically or set gain to group of harmonics 
    selected by range and n-th element formula.
- `Spectral Filter`: Applies biquad filter to the frequency bins (lowpass, highpass, bandpass, bandstop and peaking). 
    Has 4 order option (multiply by filter frequency response twice) an linear phase (multiply by magnitude response).
- `Spectral Mixer`: Mixes multiple spectral inputs with per-input level/gain and output volume control.
- `Spectral Blend`: Crossfades between two spectrums with a blend control.
- `Oscillator`: Takes a spectral input, does inverse FFT and then behaves like a wavetable oscillator.
    Up to 16 unison voices each of them is stereo. Phase and gain of each unison voices can be controlled via stereo slider.
    Controls that can be modulated: gain, pitch, frequency (through zero FM), detune, detune power (pitch distribution), 
    unison phases and unison gains blend.
- `Envelope`: AHDSR envelope generator to control both spectral and audio modules.
- `LFO`: Low-frequency oscillator (triangle/square/sine) with skew and bipolar modes.
- `Mixer`: Mixes multiple audio inputs with per-input level/gain and output volume control.
- `Waveshaper`: Wave shaping distortion (hard clip or sigmoid) with drive and clipping level.
- `Amplifier`: Simple gain modulation for input signal.
- `External Parameter`: Exposes host/plugin parameters as modulation sources with smoothing or sample-and-hold.
- `Expressions`: Use MPE as modulation sources.

## Build
```shell
cargo nih-plug bundle additizer --release
```
