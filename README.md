## Overview

Additizer is a modular synthesizer plugin. It consists of a wavetable-like oscillator
and a set of modules that process waveforms in the frequency domain.
Every slider in the UI is stereo, and each channel can be adjusted by dragging with the right mouse button.

## Modules

- `Harmonic Editor`: Draws amplitude and phase for each harmonic, and can capture a spectral input on note-on.
- `Spectral Noise`: White, pink, or brown noise in the spectrum, with cutoff and rolloff.
- `Oscillator`: Inverse-FFT wavetable oscillator with up to 16 unison voices.
- `Envelope`: AHDSR envelope.
- `Segments Envelope`: Breakpoint envelope. Length is seconds or a note value; a release point holds, then continues.
- `Multi Envelope`: Breakpoint envelope. Each segment lasts a number of seconds, and a Time input can lengthen or shorten that segment.
- `LFO`: Looping breakpoint oscillator. Rate is hertz or a note value.
- `Mod Map`: Shapes a control signal with a breakpoint curve, amount, and optional smoothing.
- `Pitch`: Note pitch with keytrack, shift, and glide.
- `Spectral Filter`: Filters a spectrum (lowpass, highpass, bandpass, notch, peak, and shelves), with keytrack and linear phase.
- `Spectral EQ`: Up to 16 spectral filter bands.
- `Band Select`: Passes a harmonic or frequency range.
- `Spectral Blend`: Crossfades two spectra.
- `Spectral Mixer`: Mixes up to six spectral inputs.
- `External Parameter`: Host parameter as a modulation source.
- `Expressions`: Velocity, gain, pan, pitch, timbre, or pressure as a modulation source.
- `Waveshaper`: Hard-clip or sigmoid distortion.
- `SVF`: Time-domain state-variable filter with cutoff, resonance, drive, and keytrack.
- `Amplifier`: Gain, level, and pan.
- `Mixer`: Mixes up to six audio inputs.
- `Output`: Master volume for the summed voices.

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
