use super::*;

use crate::{
    synth_engine::{
        DataType, ModuleInput, ModuleType,
        buffer::{HARMONIC_SERIES_BUFFER, ZEROES_SPECTRAL_BUFFER},
        routing::VoiceEvent,
        smooth::SmoothedSampleParams,
        synth_module::SynthModule,
    },
    utils::from_ms,
};

const SAMPLE_RATE: Sample = 48_000.0;

fn assert_close(a: Sample, b: Sample) {
    assert!((a - b).abs() < 1e-3, "expected {b}, got {a}");
}

// ---- A minimal Router that feeds a fixed spectrum and records outputs ----

struct TestRouter {
    spectrum: [SpectralBuffer; NUM_CHANNELS],
    outputs: Vec<(ModuleId, usize, Sample)>,
}

impl TestRouter {
    fn new(spectrum: SpectralBuffer) -> Self {
        Self {
            spectrum: [spectrum; NUM_CHANNELS],
            outputs: Vec::new(),
        }
    }

    fn with_channel_spectra(spectrum: [SpectralBuffer; NUM_CHANNELS]) -> Self {
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
        channel_idx: usize,
    ) -> Option<&SpectralBuffer> {
        Some(&self.spectrum[channel_idx])
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

/// Like [`TestRouter`], but can inject scalar and buffer modulation during `process`.
struct ModulatingTestRouter {
    spectrum: [SpectralBuffer; NUM_CHANNELS],
    outputs: Vec<(ModuleId, usize, Sample)>,
    scalar_mod: Option<(Input, Sample)>,
    buffer_mod: Option<(Input, Sample)>,
}

impl ModulatingTestRouter {
    fn new(spectrum: SpectralBuffer) -> Self {
        Self {
            spectrum: [spectrum; NUM_CHANNELS],
            outputs: Vec::new(),
            scalar_mod: None,
            buffer_mod: None,
        }
    }

    fn with_scalar_mod(mut self, input: Input, value: Sample) -> Self {
        self.scalar_mod = Some((input, value));
        self
    }

    fn with_buffer_mod(mut self, input: Input, value: Sample) -> Self {
        self.buffer_mod = Some((input, value));
        self
    }
}

impl Router for ModulatingTestRouter {
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
        input: ModuleInput,
        _voice_idx: usize,
        _channel_idx: usize,
        result: &mut [Sample],
    ) -> bool {
        if let Some((mod_input, value)) = self.buffer_mod
            && input.input_type == mod_input
        {
            for sample in result {
                *sample += value;
            }
            return true;
        }
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
        channel_idx: usize,
    ) -> Option<&SpectralBuffer> {
        Some(&self.spectrum[channel_idx])
    }

    fn get_scalar_input(
        &self,
        input: ModuleInput,
        _current: bool,
        _voice_idx: usize,
        _channel_idx: usize,
    ) -> Option<Sample> {
        self.scalar_mod
            .filter(|(mod_input, _)| input.input_type == *mod_input)
            .map(|(_, value)| value)
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

// ---- SynthModule metadata ----

#[test]
fn synth_module_metadata() {
    let osc = Oscillator::new(42);

    assert_eq!(osc.id(), 42);
    assert_eq!(osc.module_type(), ModuleType::Oscillator);
    assert_eq!(osc.output(), DataType::Buffer);
    assert!(!osc.inputs().is_empty());
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

#[test]
fn remaining_setters_round_trip_and_clamp() {
    let mut osc = Oscillator::new(1);

    osc.set_pitch_shift(StereoSample::splat(st_to_octave(100.0)));
    assert_eq!(
        osc.get_config().pitch_shift,
        StereoSample::splat(st_to_octave(60.0))
    );
    osc.set_pitch_shift(StereoSample::splat(st_to_octave(-100.0)));
    assert_eq!(
        osc.get_config().pitch_shift,
        StereoSample::splat(st_to_octave(-60.0))
    );

    osc.set_frequency_shift(StereoSample::new(12.0, -3.0));
    assert_eq!(
        osc.get_config().frequency_shift,
        StereoSample::new(12.0, -3.0)
    );

    osc.set_gains_blend(StereoSample::splat(5.0));
    assert_eq!(osc.get_config().gains_blend, StereoSample::ONE);

    osc.set_unison_phase_to(0, StereoSample::splat(5.0));
    assert_eq!(
        osc.get_config().unison[0].phase_shift_to,
        StereoSample::ONE
    );

    osc.set_unison_gain_to(1, StereoSample::splat(50.0));
    assert_eq!(osc.get_config().unison[1].gain_to, StereoSample::splat(10.0));
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

#[test]
fn handle_ui_events_applies_all_event_variants() {
    let mut osc = Oscillator::new(1);
    let mut ui = osc.ui_end.take().unwrap();

    ui.set_param(Input::PitchShift, StereoSample::splat(0.1));
    ui.set_param(Input::PhaseShift, StereoSample::splat(0.2));
    ui.set_param(Input::FrequencyShift, StereoSample::splat(5.0));
    ui.set_param(Input::Detune, StereoSample::splat(0.05));
    ui.set_param(Input::DetunePower, StereoSample::splat(-2.0));
    ui.set_param(Input::Glide, StereoSample::splat(0.5));
    ui.set_param(Input::GlideSlope, StereoSample::splat(-0.25));
    ui.set_param(Input::PhasesBlend, StereoSample::splat(0.75));
    ui.set_param(Input::GainsBlend, StereoSample::splat(0.5));
    ui.set_param(Input::Spectrum, StereoSample::ONE);
    ui.set_unison_initial_phase(0, StereoSample::splat(0.3));
    ui.set_unison_phase_shift(1, StereoSample::splat(0.4));
    ui.set_unison_phase_shift_to(2, StereoSample::splat(0.6));
    ui.set_unison_gain(0, StereoSample::splat(0.8));
    ui.set_unison_gain_to(1, StereoSample::splat(0.9));
    ui.set_unison(3);
    ui.apply_unison_level_shape(StereoSample::splat(0.5), StereoSample::splat(-6.0), true);
    ui.randomize_phases(0.1, 0.9, 0.2, PhasesDst::To);

    osc.handle_ui_events();

    let cfg = osc.get_config();
    assert_eq!(cfg.pitch_shift, StereoSample::splat(0.1));
    assert_eq!(cfg.phase_shift, StereoSample::splat(0.2));
    assert_eq!(cfg.frequency_shift, StereoSample::splat(5.0));
    assert_eq!(cfg.detune, StereoSample::splat(0.05));
    assert_eq!(cfg.detune_power, StereoSample::splat(-2.0));
    assert_eq!(cfg.glide, StereoSample::splat(0.5));
    assert_eq!(cfg.glide_slope, StereoSample::splat(-0.25));
    assert_eq!(cfg.phases_blend, StereoSample::splat(0.75));
    assert_eq!(cfg.gains_blend, StereoSample::splat(0.5));
    assert_eq!(cfg.unison[0].initial_phase, StereoSample::splat(0.3));
    assert_eq!(cfg.unison[1].phase_shift, StereoSample::splat(0.4));
    assert_eq!(cfg.unison[0].gain, StereoSample::splat(0.8));
    assert_eq!(cfg.unison_voices, 3);
    assert_close(cfg.unison[0].gain_to.left(), db_to_gain(-6.0));
    assert_close(cfg.unison[1].gain_to.left(), 1.0);
    for unison in cfg.unison.iter().take(3) {
        let phase = unison.phase_shift_to;
        assert!(phase.left() >= 0.1 && phase.left() <= 0.9);
    }
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

#[test]
fn apply_unison_level_shape_shapes_gain_to() {
    let mut osc = Oscillator::new(1);
    osc.set_unison(3);

    osc.apply_unison_level_shape(StereoSample::splat(0.5), StereoSample::splat(-6.0), true);

    let unison = osc.get_config().unison;
    let edge = db_to_gain(-6.0);

    assert_close(unison[0].gain_to.left(), edge);
    assert_close(unison[1].gain_to.left(), 1.0);
    assert_close(unison[2].gain_to.left(), edge);
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

#[test]
fn randomize_phases_writes_phase_shift_to() {
    let mut osc = Oscillator::new(1);

    osc.randomize_phases(0.2, 0.8, 0.0, PhasesDst::To);

    for unison in osc.get_config().unison.iter() {
        let phase = unison.phase_shift_to;
        assert_eq!(phase.left(), phase.right());
        assert!(phase.left() >= 0.2 && phase.left() <= 0.8);
    }
}

#[test]
fn randomize_phases_applies_stereo_spread() {
    let mut osc = Oscillator::new(1);

    osc.randomize_phases(0.0, 1.0, 0.5, PhasesDst::Initial);

    assert!(
        osc.get_config()
            .unison
            .iter()
            .any(|u| u.initial_phase.left() != u.initial_phase.right())
    );
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

#[test]
fn handle_events_applies_update_event() {
    let mut osc = Oscillator::new(1);

    osc.handle_events(&[VoiceEvent::Trigger {
        voice_idx: 0,
        prev_voice_idx: None,
        pitch: 0.5,
        velocity: 1.0,
    }]);
    osc.handle_events(&[VoiceEvent::Update {
        voice_idx: 0,
        pitch: 1.0,
        velocity: 1.0,
    }]);

    for channel in 0..NUM_CHANNELS {
        assert_eq!(osc.voices[channel][0].pitch, 1.0);
        assert_eq!(
            osc.voices[channel][0]
                .glide
                .as_ref()
                .unwrap()
                .current_pitch,
            0.5
        );
    }
}

#[test]
fn handle_events_ignores_unhandled_variants() {
    let mut osc = Oscillator::new(1);

    osc.handle_events(&[
        VoiceEvent::Release {
            voice_idx: 0,
            velocity: 0.0,
        },
        VoiceEvent::Kill { voice_idx: 0 },
    ]);

    assert!(!osc.voices[0][0].triggered);
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

#[test]
fn process_with_max_unison_voices() {
    let mut osc = Oscillator::new(1);
    osc.set_unison(MAX_UNISON_VOICES);
    osc.set_detune(StereoSample::splat(st_to_octave(0.5)));
    assert_eq!(osc.get_config().unison_voices, MAX_UNISON_VOICES);

    trigger(&mut osc, 0, 0.0);

    let mut router = TestRouter::new(HARMONIC_SERIES_BUFFER);
    let active = [0usize];
    let params = process_params(&active, 64);

    osc.process(&params, &mut router);

    let out = osc.get_buffer_output(0, 0);
    assert!(out[..64].iter().all(|s| s.is_finite()));
    assert!(out[..64].iter().any(|&s| s.abs() > 1e-6));
}

#[test]
fn process_advances_glide_when_enabled() {
    let mut osc = Oscillator::new(1);
    osc.set_glide(StereoSample::splat(1.0));

    osc.handle_events(&[VoiceEvent::Trigger {
        voice_idx: 0,
        prev_voice_idx: None,
        pitch: 0.0,
        velocity: 1.0,
    }]);
    osc.handle_events(&[VoiceEvent::Trigger {
        voice_idx: 1,
        prev_voice_idx: Some(0),
        pitch: 1.0,
        velocity: 1.0,
    }]);

    // Voice 1 starts a glide from the previous voice's pitch (0.0) toward 1.0.
    assert_eq!(osc.voices[0][1].glide.as_ref().unwrap().current_pitch, 0.0);

    let mut router = TestRouter::new(HARMONIC_SERIES_BUFFER);
    let active = [1usize];
    let params = process_params(&active, 64);

    osc.process(&params, &mut router);

    // A 64-sample block at 48 kHz is far shorter than the 1 s glide, so the
    // voice is still gliding and has moved partway toward the target pitch.
    let glide = osc.voices[0][1].glide.as_ref().expect("glide still active");
    assert!(glide.current_pitch > 0.0 && glide.current_pitch < 1.0);
}

#[test]
fn process_mono_spectrum_shares_channel_zero_waveform() {
    let mut osc = Oscillator::new(1);

    trigger(&mut osc, 0, 0.0);

    // Channel 0 gets the harmonic spectrum; channel 1 gets silence. In the mono
    // spectrum path channel 1 must reuse channel 0's waveform, so its own
    // (silent) spectrum has to be ignored.
    let mut router =
        TestRouter::with_channel_spectra([HARMONIC_SERIES_BUFFER, ZEROES_SPECTRAL_BUFFER]);
    let active = [0usize];
    let params = ProcessParams {
        samples: 64,
        sample_rate: SAMPLE_RATE,
        buffer_t_step: (64.0 as Sample).recip(),
        needs_update_ui: false,
        smooth_params: SmoothedSampleParams::new(SAMPLE_RATE),
        // Fewer spectrum channels than output channels -> mono spectrum path.
        spectrum_channels: 1,
        active_voices: &active,
    };

    osc.process(&params, &mut router);

    let left = osc.get_buffer_output(0, 0);
    let right = osc.get_buffer_output(0, 1);

    // Channel 1 reuses channel 0's waveform; with identical per-channel params
    // both channels produce the same (non-silent) output, proving channel 1's
    // own silent spectrum was ignored.
    assert!(left[..64].iter().any(|&s| s.abs() > 1e-6));
    assert_eq!(left[..64], right[..64]);
}

#[test]
fn process_second_block_reuses_swapped_waveforms() {
    let mut osc = Oscillator::new(1);
    osc.set_unison(3);
    osc.set_detune(StereoSample::splat(st_to_octave(0.25)));
    trigger(&mut osc, 0, 0.0);

    let mut router = TestRouter::new(HARMONIC_SERIES_BUFFER);
    let active = [0usize];
    let params = process_params(&active, 64);

    osc.process(&params, &mut router);
    let first = osc.get_buffer_output(0, 0)[..64].to_vec();

    osc.process(&params, &mut router);
    let second = osc.get_buffer_output(0, 0)[..64].to_vec();

    assert!(first.iter().all(|s| s.is_finite()));
    assert!(second.iter().all(|s| s.is_finite()));
    assert!(first.iter().any(|&s| s.abs() > 1e-6));
    assert_ne!(first, second);
}

#[test]
fn process_sub_ms_glide_is_cancelled() {
    let mut osc = Oscillator::new(1);
    osc.set_glide(StereoSample::splat(from_ms(0.5)));

    osc.handle_events(&[VoiceEvent::Trigger {
        voice_idx: 0,
        prev_voice_idx: None,
        pitch: 0.0,
        velocity: 1.0,
    }]);
    osc.handle_events(&[VoiceEvent::Trigger {
        voice_idx: 1,
        prev_voice_idx: Some(0),
        pitch: 1.0,
        velocity: 1.0,
    }]);

    assert!(osc.voices[0][1].glide.is_some());

    let mut router = TestRouter::new(HARMONIC_SERIES_BUFFER);
    let active = [1usize];
    let params = process_params(&active, 64);

    osc.process(&params, &mut router);

    assert!(osc.voices[0][1].glide.is_none());
}

#[test]
fn process_exponential_glide_differs_from_linear() {
    let mut linear = Oscillator::new(1);
    linear.set_glide(StereoSample::splat(1.0));
    linear.set_glide_slope(StereoSample::ZERO);

    let mut curved = Oscillator::new(1);
    curved.set_glide(StereoSample::splat(1.0));
    curved.set_glide_slope(StereoSample::splat(-0.5));

    for osc in [&mut linear, &mut curved] {
        osc.handle_events(&[VoiceEvent::Trigger {
            voice_idx: 0,
            prev_voice_idx: None,
            pitch: 0.0,
            velocity: 1.0,
        }]);
        osc.handle_events(&[VoiceEvent::Trigger {
            voice_idx: 1,
            prev_voice_idx: Some(0),
            pitch: 1.0,
            velocity: 1.0,
        }]);
    }

    let mut router = TestRouter::new(HARMONIC_SERIES_BUFFER);
    let active = [1usize];
    let params = process_params(&active, 64);

    linear.process(&params, &mut router);
    curved.process(&params, &mut TestRouter::new(HARMONIC_SERIES_BUFFER));

    let linear_pitch = linear.voices[0][1]
        .glide
        .as_ref()
        .map(|g| g.current_pitch)
        .unwrap_or(1.0);
    let curved_pitch = curved.voices[0][1]
        .glide
        .as_ref()
        .map(|g| g.current_pitch)
        .unwrap_or(1.0);

    assert!(linear_pitch > 0.0 && linear_pitch < 1.0);
    assert!(curved_pitch > 0.0 && curved_pitch < 1.0);
    assert!((linear_pitch - curved_pitch).abs() > 1e-4);
}

#[test]
fn process_glide_completes_mid_block() {
    let mut osc = Oscillator::new(1);
    // ~2 samples at 48 kHz — shorter than the 64-sample block.
    osc.set_glide(StereoSample::splat(2.0 / SAMPLE_RATE));

    osc.handle_events(&[VoiceEvent::Trigger {
        voice_idx: 0,
        prev_voice_idx: None,
        pitch: 0.0,
        velocity: 1.0,
    }]);
    osc.handle_events(&[VoiceEvent::Trigger {
        voice_idx: 1,
        prev_voice_idx: Some(0),
        pitch: 1.0,
        velocity: 1.0,
    }]);

    let mut router = TestRouter::new(HARMONIC_SERIES_BUFFER);
    let active = [1usize];
    let params = process_params(&active, 64);

    osc.process(&params, &mut router);

    assert!(osc.voices[0][1].glide.is_none());
}

#[test]
fn process_with_scalar_detune_modulation_differs_from_unmodulated() {
    let mut unmodulated = Oscillator::new(1);
    unmodulated.set_unison(3);
    unmodulated.set_detune(StereoSample::splat(st_to_octave(0.25)));

    let mut modulated = Oscillator::new(1);
    modulated.set_unison(3);
    modulated.set_detune(StereoSample::splat(st_to_octave(0.25)));

    trigger(&mut unmodulated, 0, 0.0);
    trigger(&mut modulated, 0, 0.0);

    let active = [0usize];
    let params = process_params(&active, 64);

    unmodulated.process(
        &params,
        &mut TestRouter::new(HARMONIC_SERIES_BUFFER),
    );
    modulated.process(
        &params,
        &mut ModulatingTestRouter::new(HARMONIC_SERIES_BUFFER)
            .with_scalar_mod(Input::Detune, st_to_octave(0.1)),
    );

    let plain = unmodulated.get_buffer_output(0, 0)[..64].to_vec();
    let detuned = modulated.get_buffer_output(0, 0)[..64].to_vec();

    assert_ne!(plain, detuned);
}

#[test]
fn process_with_gain_buffer_modulation_scales_output() {
    let mut quiet = Oscillator::new(1);
    quiet.set_gain(StereoSample::splat(0.5));
    trigger(&mut quiet, 0, 0.0);

    let mut boosted = Oscillator::new(1);
    boosted.set_gain(StereoSample::splat(0.5));
    trigger(&mut boosted, 0, 0.0);

    let active = [0usize];
    let params = process_params(&active, 64);

    quiet.process(
        &params,
        &mut TestRouter::new(HARMONIC_SERIES_BUFFER),
    );
    boosted.process(
        &params,
        &mut ModulatingTestRouter::new(HARMONIC_SERIES_BUFFER)
            .with_buffer_mod(Input::Gain, 0.5),
    );

    let quiet_rms: Sample = quiet.get_buffer_output(0, 0)[..64]
        .iter()
        .map(|s| s * s)
        .sum::<Sample>()
        .sqrt();
    let boosted_rms: Sample = boosted.get_buffer_output(0, 0)[..64]
        .iter()
        .map(|s| s * s)
        .sum::<Sample>()
        .sqrt();

    assert!(boosted_rms > quiet_rms);
}
