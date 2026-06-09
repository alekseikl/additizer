use super::*;

use crate::synth_engine::{
    ModuleInput,
    buffer::{HARMONIC_SERIES_BUFFER, ZEROES_SPECTRAL_BUFFER},
    routing::VoiceEvent,
    smooth::SmoothedSampleParams,
};

const SAMPLE_RATE: Sample = 48_000.0;

fn assert_close(a: Sample, b: Sample) {
    assert!((a - b).abs() < 1e-3, "expected {b}, got {a}");
}

// ---- A minimal Router that feeds a fixed spectrum and records outputs ----

struct TestRouter {
    spectrum: SpectralBuffer,
    outputs: Vec<(ModuleId, usize, Sample)>,
}

impl TestRouter {
    fn new(spectrum: SpectralBuffer) -> Self {
        Self {
            spectrum,
            outputs: Vec::new(),
        }
    }
}

impl Router for TestRouter {
    fn get_input<'a>(
        &'a self,
        _input: ModuleInput,
        _samples: usize,
        _voice_idx: usize,
        _channel_idx: usize,
        _input_buffer: &'a mut Buffer,
    ) -> Option<&'a Buffer> {
        None
    }

    fn add_input_to(
        &self,
        _input: ModuleInput,
        _voice_idx: usize,
        _channel_idx: usize,
        _result: &mut [Sample],
    ) -> bool {
        false
    }

    fn read_unmodulated_input(
        &self,
        _input: ModuleInput,
        samples: usize,
        _voice_idx: usize,
        _channel_idx: usize,
        input_buffer: &mut Buffer,
    ) {
        input_buffer[..samples].fill(0.0);
    }

    fn get_spectral_input(
        &self,
        _input: ModuleInput,
        _current: bool,
        _voice_idx: usize,
        _channel_idx: usize,
    ) -> Option<&SpectralBuffer> {
        Some(&self.spectrum)
    }

    fn get_scalar_input(
        &self,
        _input: ModuleInput,
        _current: bool,
        _voice_idx: usize,
        _channel_idx: usize,
    ) -> Option<Sample> {
        None
    }

    fn update_modulated_input(
        &mut self,
        _module_id: ModuleId,
        _input: Input,
        _channel_idx: usize,
        _value: Sample,
    ) {
    }

    fn update_output(&mut self, module_id: ModuleId, channel_idx: usize, value: Sample) {
        self.outputs.push((module_id, channel_idx, value));
    }
}

fn process_params(active: &[usize], samples: usize) -> ProcessParams<'_> {
    ProcessParams {
        samples,
        sample_rate: SAMPLE_RATE,
        buffer_t_step: (samples as Sample).recip(),
        needs_update_ui: false,
        smooth_params: SmoothedSampleParams::new(SAMPLE_RATE),
        spectrum_channels: NUM_CHANNELS,
        active_voices: active,
    }
}

fn trigger(osc: &mut Oscillator, voice_idx: usize, pitch: Sample) {
    osc.handle_events(&[VoiceEvent::Trigger {
        voice_idx,
        prev_voice_idx: None,
        pitch,
        velocity: 1.0,
    }]);
}

// ---- Construction & config round-trip ----

#[test]
fn config_round_trips() {
    let cfg = OscillatorConfig {
        id: 7,
        unison_voices: 5,
        steal_phase: true,
        gain: StereoSample::new(0.4, 0.6),
        pitch_shift: StereoSample::splat(0.25),
        detune: StereoSample::splat(0.05),
        detune_power: StereoSample::new(1.0, -2.0),
        ..OscillatorConfig::default()
    };

    let got = Oscillator::from_config(&cfg).get_config();

    assert_eq!(got.id, 7);
    assert_eq!(got.unison_voices, 5);
    assert!(got.steal_phase);
    assert_eq!(got.gain, StereoSample::new(0.4, 0.6));
    assert_eq!(got.pitch_shift, StereoSample::splat(0.25));
    assert_eq!(got.detune, StereoSample::splat(0.05));
    assert_eq!(got.detune_power, StereoSample::new(1.0, -2.0));
}

#[test]
fn new_uses_given_id_and_defaults() {
    let osc = Oscillator::new(3);
    let cfg = osc.get_config();

    assert_eq!(cfg.id, 3);
    assert_eq!(cfg.unison_voices, 1);
    assert!(!cfg.steal_phase);
    assert_eq!(cfg.gain, StereoSample::ONE);
}

// ---- Parameter setters & clamping ----

#[test]
fn setters_clamp_to_valid_ranges() {
    let mut osc = Oscillator::new(1);

    osc.set_gain(StereoSample::splat(2.0));
    assert_eq!(osc.get_config().gain, StereoSample::ONE);
    osc.set_gain(StereoSample::splat(-1.0));
    assert_eq!(osc.get_config().gain, StereoSample::ZERO);

    osc.set_detune(StereoSample::splat(10.0));
    assert_eq!(
        osc.get_config().detune,
        StereoSample::splat(st_to_octave(1.0))
    );

    osc.set_detune_power(StereoSample::splat(100.0));
    assert_eq!(osc.get_config().detune_power, StereoSample::splat(5.0));

    osc.set_glide(StereoSample::splat(100.0));
    assert_eq!(osc.get_config().glide, StereoSample::splat(MAX_GLIDE));

    osc.set_glide_slope(StereoSample::splat(-100.0));
    assert_eq!(osc.get_config().glide_slope, StereoSample::splat(-1.0));

    osc.set_phase_shift(StereoSample::splat(5.0));
    assert_eq!(osc.get_config().phase_shift, StereoSample::ONE);

    osc.set_phases_blend(StereoSample::splat(5.0));
    assert_eq!(osc.get_config().phases_blend, StereoSample::ONE);
}

#[test]
fn set_unison_clamps_voice_count() {
    let mut osc = Oscillator::new(1);

    osc.set_unison(0);
    assert_eq!(osc.get_config().unison_voices, 1);

    osc.set_unison(999);
    assert_eq!(osc.get_config().unison_voices, MAX_UNISON_VOICES);

    osc.set_unison(8);
    assert_eq!(osc.get_config().unison_voices, 8);
}

#[test]
fn unison_setters_clamp() {
    let mut osc = Oscillator::new(1);

    osc.set_unison_gain(0, StereoSample::splat(50.0));
    assert_eq!(osc.get_config().unison[0].gain, StereoSample::splat(10.0));

    osc.set_initial_phase(2, StereoSample::splat(5.0));
    assert_eq!(osc.get_config().unison[2].initial_phase, StereoSample::ONE);

    osc.set_unison_phase(1, StereoSample::splat(-9.0));
    assert_eq!(
        osc.get_config().unison[1].phase_shift,
        StereoSample::splat(-1.0)
    );
}

// ---- handle_ui_events ----

#[test]
fn handle_ui_events_applies_param_events() {
    let mut osc = Oscillator::new(1);
    let mut ui = osc.ui_end.take().unwrap();

    ui.set_param(Input::Gain, StereoSample::splat(0.3));
    ui.set_unison(4);
    ui.set_steal_phase(true);

    osc.handle_ui_events();

    let cfg = osc.get_config();
    assert_eq!(cfg.gain, StereoSample::splat(0.3));
    assert_eq!(cfg.unison_voices, 4);
    assert!(cfg.steal_phase);
}

// ---- apply_unison_level_shape ----

#[test]
fn apply_unison_level_shape_is_noop_below_two_voices() {
    let mut osc = Oscillator::new(1);
    osc.set_unison(1);

    osc.apply_unison_level_shape(StereoSample::splat(0.5), StereoSample::splat(-6.0), false);

    assert_eq!(osc.get_config().unison[0].gain, StereoSample::ONE);
}

#[test]
fn apply_unison_level_shape_shapes_gains() {
    let mut osc = Oscillator::new(1);
    osc.set_unison(3);

    osc.apply_unison_level_shape(StereoSample::splat(0.5), StereoSample::splat(-6.0), false);

    let unison = osc.get_config().unison;
    let edge = db_to_gain(-6.0);

    // Edges sit at the supplied level, the centre is left untouched (0 dB).
    assert_close(unison[0].gain.left(), edge);
    assert_close(unison[1].gain.left(), 1.0);
    assert_close(unison[2].gain.left(), edge);
}

// ---- randomize_phases ----

#[test]
fn randomize_phases_writes_into_requested_destination() {
    let mut osc = Oscillator::new(1);

    osc.randomize_phases(0.2, 0.8, 0.0, PhasesDst::Initial);

    for unison in osc.get_config().unison.iter() {
        let phase = unison.initial_phase;
        // No stereo spread -> both channels share the same value.
        assert_eq!(phase.left(), phase.right());
        assert!(phase.left() >= 0.2 && phase.left() <= 0.8);
    }
}

#[test]
fn randomize_phases_is_deterministic() {
    let mut a = Oscillator::new(1);
    let mut b = Oscillator::new(1);

    a.randomize_phases(0.0, 1.0, 0.3, PhasesDst::From);
    b.randomize_phases(0.0, 1.0, 0.3, PhasesDst::From);

    let ua = a.get_config().unison;
    let ub = b.get_config().unison;

    for (x, y) in ua.iter().zip(ub.iter()) {
        assert_eq!(x.phase_shift, y.phase_shift);
    }
}

// ---- Triggering & voice state ----

#[test]
fn handle_trigger_sets_voice_state_and_initial_phases() {
    let mut osc = Oscillator::new(1);

    osc.handle_trigger(0, None, 0, 0.5);

    let voice = &osc.voices[0][0];
    assert!(voice.triggered);
    assert_eq!(voice.pitch, 0.5);
    assert!(voice.glide.is_none());

    let defaults = OscillatorConfig::default();
    assert_close(
        voice.phases[0].normalized(),
        defaults.unison[0].initial_phase.left(),
    );
    assert_close(
        voice.phases[1].normalized(),
        defaults.unison[1].initial_phase.left(),
    );
}

#[test]
fn handle_trigger_with_prev_voice_starts_glide() {
    let mut osc = Oscillator::new(1);

    osc.handle_trigger(0, None, 0, 0.0);
    osc.handle_trigger(0, Some(0), 1, 1.0);

    let voice = &osc.voices[0][1];
    let glide = voice.glide.as_ref().expect("glide should be created");
    assert_eq!(glide.current_pitch, 0.0);
    assert_eq!(voice.pitch, 1.0);
}

#[test]
fn handle_trigger_steals_phases_when_enabled() {
    let cfg = OscillatorConfig {
        id: 1,
        steal_phase: true,
        ..OscillatorConfig::default()
    };
    let mut osc = Oscillator::from_config(&cfg);

    osc.handle_trigger(0, None, 0, 0.0);
    osc.handle_trigger(0, Some(0), 1, 1.0);

    assert_eq!(osc.voices[0][1].phases, osc.voices[0][0].phases);
}

#[test]
fn handle_update_glides_from_current_pitch() {
    let mut osc = Oscillator::new(1);

    osc.handle_trigger(0, None, 0, 0.5);
    osc.handle_update(0, 0, 1.0);

    let voice = &osc.voices[0][0];
    assert_eq!(voice.pitch, 1.0);
    assert_eq!(voice.glide.as_ref().unwrap().current_pitch, 0.5);
}

#[test]
fn handle_events_applies_to_all_channels() {
    let mut osc = Oscillator::new(1);

    osc.handle_events(&[VoiceEvent::Trigger {
        voice_idx: 2,
        prev_voice_idx: None,
        pitch: 0.25,
        velocity: 1.0,
    }]);

    for channel in 0..NUM_CHANNELS {
        assert!(osc.voices[channel][2].triggered);
        assert_eq!(osc.voices[channel][2].pitch, 0.25);
    }
}

// ---- Static waveform helpers ----

#[test]
fn wrap_wave_buffer_mirrors_padding() {
    let mut buf = make_zero_wave_buffer();

    buf[WAVEFORM_BUFFER_SIZE - WAVEFORM_PAD_RIGHT - 1] = 5.0;
    buf[WAVEFORM_PAD_LEFT] = 7.0;
    buf[WAVEFORM_PAD_LEFT + 1] = 9.0;

    Oscillator::wrap_wave_buffer(&mut buf);

    assert_eq!(buf[0], 5.0);
    assert_eq!(buf[WAVEFORM_BUFFER_SIZE - WAVEFORM_PAD_RIGHT], 7.0);
    assert_eq!(buf[WAVEFORM_BUFFER_SIZE - WAVEFORM_PAD_RIGHT + 1], 9.0);
}

#[test]
fn load_segment_reads_four_consecutive_samples() {
    let mut buf = make_zero_wave_buffer();
    buf[10] = 1.0;
    buf[11] = 2.0;
    buf[12] = 3.0;
    buf[13] = 4.0;

    let seg = Oscillator::load_segment(&buf, 10);

    assert_eq!(seg.to_array(), [1.0, 2.0, 3.0, 4.0]);
}

#[test]
fn interpolated_segment_blends_between_buffers() {
    let from = make_zero_wave_buffer();
    let mut to = make_zero_wave_buffer();
    to.fill(2.0);

    // buff_t = 0.5 -> each coefficient is halfway between 0 and 2, i.e. 1.0.
    // t = 0 -> Catmull-Rom polynomial selects only the second sample.
    let seg = Oscillator::interpolated_segment(&from, &to, 0.5, 10, 0.0);

    assert_eq!(seg.to_array(), [0.0, 1.0, 0.0, 0.0]);
}

// ---- Full process path ----

#[test]
fn process_harmonic_spectrum_produces_audio() {
    let mut osc = Oscillator::new(1);
    trigger(&mut osc, 0, 0.0);

    let mut router = TestRouter::new(HARMONIC_SERIES_BUFFER);
    let active = [0usize];
    let params = process_params(&active, 64);

    osc.process(&params, &mut router);

    let out = osc.get_buffer_output(0, 0);
    assert!(out[..64].iter().all(|s| s.is_finite()));
    assert!(out[..64].iter().any(|&s| s.abs() > 1e-6));
    assert_eq!(router.outputs.len(), NUM_CHANNELS);
}

#[test]
fn process_silent_spectrum_is_silent() {
    let mut osc = Oscillator::new(1);
    trigger(&mut osc, 0, 0.0);

    let mut router = TestRouter::new(ZEROES_SPECTRAL_BUFFER);
    let active = [0usize];
    let params = process_params(&active, 64);

    osc.process(&params, &mut router);

    let out = osc.get_buffer_output(0, 0);
    assert!(out[..64].iter().all(|&s| s == 0.0));
}
