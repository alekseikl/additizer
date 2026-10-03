use super::{Output, SynthModule, Voice};
use crate::synth_engine::{
    DataType, EngineAudioEnd, EngineConfig, EngineParams, Input, LinkConfig, ModuleConfig,
    ModuleId, NUM_CHANNELS, Note, OUTPUT_MODULE_ID, OutputMeter, ProcessContext, ProcessParams,
    Sample, StereoSample, SynthEngine, VoiceEvent,
    buffer::BUFFER_SIZE,
    engine_io::UiEvent,
    from_ms,
    harmonic_editor::HarmonicEditorConfig,
    iir_decimator::IirDecimator,
    oscillator::OscillatorConfig,
    routing::{InputSlot, InputSlots, OutputsArena, SpectralInputSlot},
    smooth::SmoothedSampleParams,
    voices_handler::{DecayingVoice, PlayingVoice, VoicesHandlerMetrics},
};
use addi_dsp::db_to_gain_fast;

const SAMPLE_RATE: Sample = 48_000.0;
const HARMONIC_EDITOR_ID: ModuleId = 1;
const OSCILLATOR_ID: ModuleId = 2;

fn assert_close(a: Sample, b: Sample) {
    assert!((a - b).abs() <= 1e-4, "{a} ≈ {b}");
}

fn voice_mut(output: &mut Output, channel: usize, index: usize) -> &mut Voice {
    &mut output.channels[channel].voices[index]
}

fn kill_event(voice_idx: usize, offset: usize) -> VoiceEvent {
    VoiceEvent::Kill { voice_idx, offset }
}

fn reset_event(voice_idx: usize) -> VoiceEvent {
    VoiceEvent::Reset {
        voice_idx,
        replaced_voice_idx: None,
        prev_note: None,
        pitch: 0.0,
        velocity: 1.0,
        offset: 0,
    }
}

struct RecordingAudioEnd {
    meter: OutputMeter,
}

impl EngineAudioEnd for RecordingAudioEnd {
    fn update_modulated_input(
        &mut self,
        _module_id: ModuleId,
        _input: Input,
        _channel: u8,
        _value: Sample,
        _normalized_value: Sample,
    ) -> bool {
        false
    }

    fn update_voices_status(&mut self, _metrics: &VoicesHandlerMetrics) -> bool {
        false
    }

    fn pop_event(&mut self) -> Option<UiEvent> {
        None
    }

    fn update_out_volume(&mut self, volume: StereoSample, clipped: [bool; NUM_CHANNELS]) {
        self.meter = OutputMeter { volume, clipped };
    }
}

fn run_process(
    output: &mut Output,
    samples: usize,
    trigger_stage: bool,
    update_ui: bool,
) -> OutputMeter {
    let mut arena = OutputsArena::new();
    let mut audio_end = RecordingAudioEnd {
        meter: OutputMeter::default(),
    };
    let active: &[PlayingVoice] = &[];

    {
        let mut ctx = ProcessContext {
            outputs_arena: &mut arena,
            audio_end: &mut audio_end,
            params: ProcessParams {
                trigger_stage,
                has_triggered_voices: false,
                samples,
                sample_rate: SAMPLE_RATE,
                needs_update_ui: update_ui,
                smooth_params: SmoothedSampleParams::new(SAMPLE_RATE),
                active_voices: active,
            },
        };

        output.process(&mut ctx);
    }

    audio_end.meter
}

fn make_engine(engine: EngineParams, link_output: bool) -> SynthEngine {
    let mut links = vec![LinkConfig::direct(
        HARMONIC_EDITOR_ID,
        OSCILLATOR_ID,
        Input::Spectrum,
    )];

    if link_output {
        links.push(LinkConfig::direct(
            OSCILLATOR_ID,
            OUTPUT_MODULE_ID,
            Input::Audio,
        ));
    }

    let config = EngineConfig {
        engine,
        modules: vec![
            ModuleConfig::HarmonicEditor(Box::new(HarmonicEditorConfig {
                id: HARMONIC_EDITOR_ID,
                ..HarmonicEditorConfig::default()
            })),
            ModuleConfig::Oscillator(Box::new(OscillatorConfig {
                id: OSCILLATOR_ID,
                ..OscillatorConfig::default()
            })),
        ],
        links,
    };

    SynthEngine::try_new(&config, SAMPLE_RATE).expect("valid engine config")
}

fn note(midi: u8) -> Note {
    Note {
        channel: 0,
        note: midi,
        velocity: 1.0,
        host_id: None,
    }
}

fn process_block(engine: &mut SynthEngine, samples: usize) -> (Vec<Sample>, Vec<Sample>) {
    let mut left = vec![0.0; samples];
    let mut right = vec![0.0; samples];
    let mut terminated = Vec::new();

    engine.process(
        samples,
        false,
        &mut terminated,
        [&mut left[..], &mut right[..]],
    );

    (left, right)
}

fn play(engine: &mut SynthEngine, midi: u8) -> (Vec<Sample>, Vec<Sample>) {
    engine.handle_note_on(note(midi), 0);
    process_block(engine, 64)
}

#[test]
fn gain_and_kill_time_are_clamped() {
    let mut output = Output::new(StereoSample::new(-1.0, 8.0), from_ms(80.0));

    assert_eq!(output.get_gain(), StereoSample::new(0.0, 4.0));
    assert_eq!(output.get_voice_kill_time(), from_ms(50.0));

    output.set_gain(StereoSample::new(1.5, -0.25));
    output.set_voice_kill_time(-1.0);

    assert_eq!(output.get_gain(), StereoSample::new(1.5, 0.0));
    assert_eq!(output.get_voice_kill_time(), 0.0);

    output.set_voice_kill_time(from_ms(12.0));
    assert_eq!(output.get_voice_kill_time(), from_ms(12.0));
}

#[test]
fn module_identity_is_fixed() {
    let output = Output::new(StereoSample::ONE, 0.0);

    assert_eq!(output.id(), OUTPUT_MODULE_ID);
    assert_eq!(output.output_type(), DataType::Audio);
    assert_eq!(output.output_slot(), usize::MAX);

    let inputs = output.inputs();
    assert_eq!(inputs.len(), 1);
    assert_eq!(inputs[0].input_type, Input::Audio);
    assert_eq!(inputs[0].data_type, DataType::Audio);
    assert!(inputs[0].is_direct);
}

#[test]
#[should_panic(expected = "Output module doesn't have output slot.")]
fn set_output_slot_panics() {
    let mut output = Output::new(StereoSample::ONE, 0.0);
    output.set_output_slot(0);
}

#[test]
fn audio_input_slot_comes_from_the_first_direct_source() {
    let mut output = Output::new(StereoSample::ONE, 0.0);
    let slots = InputSlots {
        input_type: Input::Audio,
        slots: vec![
            InputSlot {
                src_slot: 4,
                modulation_slot: None,
                amount: StereoSample::ONE,
            },
            InputSlot {
                src_slot: 9,
                modulation_slot: Some(1),
                amount: StereoSample::ONE,
            },
        ],
    };

    output.set_input_slots(
        &[slots],
        &[SpectralInputSlot {
            input_type: Input::Spectrum,
            slot: 2,
        }],
    );
    assert_eq!(output.audio_input, Some(4));

    output.set_input_slots(
        &[InputSlots {
            input_type: Input::Audio,
            slots: Vec::new(),
        }],
        &[],
    );
    assert_eq!(output.audio_input, None);

    output.set_input_slots(&[], &[]);
    assert_eq!(output.audio_input, None);
}

#[test]
fn kill_and_reset_events_update_every_channel() {
    let mut output = Output::new(StereoSample::ONE, from_ms(10.0));

    output.process_events(&[
        kill_event(2, 6),
        VoiceEvent::Update {
            voice_idx: 2,
            pitch: 1.0,
            velocity: 0.5,
            offset: 1,
        },
        VoiceEvent::Release {
            voice_idx: 2,
            velocity: 0.0,
            offset: 3,
        },
    ]);

    for channel in 0..NUM_CHANNELS {
        let voice = voice_mut(&mut output, channel, 2);
        assert!(voice.killing);
        assert_eq!(voice.killing_offset, Some(6));
        assert_eq!(voice.killing_time, 0.0);
    }

    voice_mut(&mut output, 0, 2).killing_time = 0.02;
    voice_mut(&mut output, 1, 2).killing_time = 0.02;
    output.process_events(&[reset_event(2), kill_event(1, 0)]);

    for channel in 0..NUM_CHANNELS {
        let reset = &output.channels[channel].voices[2];
        assert!(!reset.killing);
        assert_eq!(reset.killing_offset, None);
        assert_eq!(reset.killing_time, 0.0);

        let killed = &output.channels[channel].voices[1];
        assert!(killed.killing);
        assert_eq!(killed.killing_offset, Some(0));
    }
}

#[test]
fn poll_ignores_voices_that_are_already_done() {
    let output = Output::new(StereoSample::ONE, from_ms(10.0));
    let mut decaying = [DecayingVoice::new(0)];

    output.poll_decaying_voices(&mut decaying);

    assert!(decaying[0].is_done());
}

#[test]
fn poll_keeps_an_active_voice_until_the_kill_fade_finishes() {
    let kill_time = from_ms(10.0);
    let mut output = Output::new(StereoSample::ONE, kill_time);

    let mut active = [DecayingVoice::new(3)];
    active[0].mark_active();
    output.poll_decaying_voices(&mut active);
    assert!(!active[0].is_done());

    for channel in 0..NUM_CHANNELS {
        let voice = voice_mut(&mut output, channel, 3);
        voice.killing = true;
        voice.killing_time = kill_time * 0.5;
    }
    let mut fading = [DecayingVoice::new(3)];
    fading[0].mark_active();
    output.poll_decaying_voices(&mut fading);
    assert!(!fading[0].is_done());

    for channel in 0..NUM_CHANNELS {
        voice_mut(&mut output, channel, 3).killing_offset = Some(4);
        voice_mut(&mut output, channel, 3).killing_time = kill_time;
    }
    let mut pending = [DecayingVoice::new(3)];
    pending[0].mark_active();
    output.poll_decaying_voices(&mut pending);
    assert!(!pending[0].is_done());

    for channel in 0..NUM_CHANNELS {
        let voice = voice_mut(&mut output, channel, 3);
        voice.killing_offset = None;
        voice.killing_time = kill_time;
    }
    let mut finished = [DecayingVoice::new(3)];
    finished[0].mark_active();
    output.poll_decaying_voices(&mut finished);
    assert!(finished[0].is_done());
}

#[test]
fn poll_stays_active_when_one_channel_is_still_fading() {
    let kill_time = from_ms(10.0);
    let mut output = Output::new(StereoSample::ONE, kill_time);

    for channel in 0..NUM_CHANNELS {
        let voice = voice_mut(&mut output, channel, 1);
        voice.killing = true;
        voice.killing_time = kill_time;
    }
    voice_mut(&mut output, 1, 1).killing_time = kill_time * 0.25;

    let mut decaying = [DecayingVoice::new(1)];
    decaying[0].mark_active();
    output.poll_decaying_voices(&mut decaying);

    assert!(!decaying[0].is_done());
}

#[test]
fn kill_fade_is_a_no_op_until_the_voice_is_killing() {
    let mut output = Output::new(StereoSample::ONE, from_ms(10.0));
    let voice = voice_mut(&mut output, 0, 0);
    voice.killing_offset = Some(2);
    let mut buffer = [0.4; 4];

    Output::apply_kill_fade(&mut buffer, voice, from_ms(10.0), SAMPLE_RATE);

    assert_eq!(buffer, [0.4; 4]);
    assert_eq!(voice.killing_offset, Some(2));
    assert_eq!(voice.killing_time, 0.0);
}

#[test]
fn short_kill_time_mutes_from_the_offset() {
    let mut output = Output::new(StereoSample::ONE, 0.0);
    output.process_events(&[kill_event(0, 3)]);

    let voice = voice_mut(&mut output, 0, 0);
    let mut buffer = [1.0; 8];
    Output::apply_kill_fade(&mut buffer, voice, 0.0, SAMPLE_RATE);

    assert_eq!(&buffer[..3], &[1.0, 1.0, 1.0]);
    assert!(buffer[3..].iter().all(|sample| *sample == 0.0));
    assert_eq!(voice.killing_offset, None);
    assert_eq!(voice.killing_time, 0.0);

    let voice = voice_mut(&mut output, 1, 0);
    voice.killing_offset = None;
    let mut muted = [1.0; 4];
    Output::apply_kill_fade(
        &mut muted,
        voice,
        super::INSTANT_KILL_TIME * 0.5,
        SAMPLE_RATE,
    );
    assert_eq!(muted, [0.0; 4]);
    assert_eq!(voice.killing_time, super::INSTANT_KILL_TIME * 0.5);

    let mut curve = Output::new(StereoSample::ONE, super::INSTANT_KILL_TIME);
    let voice = voice_mut(&mut curve, 0, 0);
    voice.killing = true;
    let mut boundary = [1.0; 2];
    Output::apply_kill_fade(&mut boundary, voice, super::INSTANT_KILL_TIME, SAMPLE_RATE);
    assert_eq!(boundary[0], 1.0);
    assert!(boundary[1] < 1.0);
}

#[test]
fn kill_fade_starts_at_unity_and_reaches_silence() {
    let kill_time = from_ms(2.0);
    let mut output = Output::new(StereoSample::ONE, kill_time);
    let voice = voice_mut(&mut output, 0, 0);
    voice.killing = true;
    voice.killing_offset = Some(2);

    let fade_samples = (kill_time * SAMPLE_RATE).round() as usize;
    let mut buffer = vec![1.0; fade_samples + 3];
    Output::apply_kill_fade(&mut buffer, voice, kill_time, SAMPLE_RATE);

    assert_eq!(&buffer[..2], &[1.0, 1.0]);
    assert_eq!(buffer[2], 1.0);
    assert!(
        buffer[3..fade_samples + 2]
            .windows(2)
            .all(|pair| pair[0] > pair[1])
    );
    assert!(buffer[fade_samples + 2].abs() < 1e-5);
    assert!(buffer[fade_samples / 2 + 2] < 0.2);
    assert_eq!(voice.killing_offset, None);
}

#[test]
fn kill_fade_continues_across_calls() {
    let kill_time = from_ms(5.0);
    let mut continued = Output::new(StereoSample::ONE, kill_time);
    let mut once = Output::new(StereoSample::ONE, kill_time);

    for output in [&mut continued, &mut once] {
        voice_mut(output, 0, 0).killing = true;
    }

    let mut continued_buffer = [1.0; 40];
    Output::apply_kill_fade(
        &mut continued_buffer[..10],
        voice_mut(&mut continued, 0, 0),
        kill_time,
        SAMPLE_RATE,
    );
    Output::apply_kill_fade(
        &mut continued_buffer[10..],
        voice_mut(&mut continued, 0, 0),
        kill_time,
        SAMPLE_RATE,
    );

    let mut once_buffer = [1.0; 40];
    Output::apply_kill_fade(
        &mut once_buffer,
        voice_mut(&mut once, 0, 0),
        kill_time,
        SAMPLE_RATE,
    );

    for (continued, once) in continued_buffer.iter().zip(once_buffer) {
        assert_close(*continued, once);
    }
}

#[test]
fn kill_at_the_end_of_the_block_waits_for_the_next_block() {
    let kill_time = from_ms(10.0);
    let mut output = Output::new(StereoSample::ONE, kill_time);
    let voice = voice_mut(&mut output, 0, 0);
    voice.killing = true;
    voice.killing_offset = Some(4);

    let mut buffer = [1.0; 4];
    Output::apply_kill_fade(&mut buffer, voice, kill_time, SAMPLE_RATE);

    assert_eq!(buffer, [1.0; 4]);
    assert_eq!(voice.killing_time, 0.0);
    assert_eq!(voice.killing_offset, None);

    let mut next = [1.0; 2];
    Output::apply_kill_fade(&mut next, voice, kill_time, SAMPLE_RATE);
    assert_eq!(next[0], 1.0);
    assert!(next[1] < 1.0);
}

#[test]
fn volume_scales_and_clips_at_12_db() {
    let clip = db_to_gain_fast(super::OUTPUT_CLIP_DB);
    let mut buffer = [0.5, -0.25, clip, -clip];
    let clipped = Output::apply_volume(buffer.iter_mut(), std::iter::repeat(1.0), 4);

    assert!(!clipped);
    assert_eq!(buffer, [0.5, -0.25, clip, -clip]);

    let mut hot = [clip + 0.25, -(clip + 0.5), 0.1];
    let clipped = Output::apply_volume(hot.iter_mut(), std::iter::repeat(1.0), 2);

    assert!(clipped);
    assert_eq!(hot[0], clip);
    assert_eq!(hot[1], -clip);
    assert_eq!(hot[2], 0.1);

    let mut scaled = [0.5, -0.25, 1.0];
    let clipped = Output::apply_volume(scaled.iter_mut(), std::iter::repeat(2.0), 2);

    assert!(!clipped);
    assert_eq!(scaled, [1.0, -0.5, 1.0]);
}

#[test]
fn volume_replaces_non_finite_samples_with_silence() {
    let clip = db_to_gain_fast(super::OUTPUT_CLIP_DB);
    let mut buffer = [Sample::NAN, Sample::INFINITY, Sample::NEG_INFINITY, 0.25];
    let clipped = Output::apply_volume(buffer.iter_mut(), std::iter::repeat(1.0), 4);

    assert!(clipped);
    assert_eq!(buffer[0], 0.0);
    assert_eq!(buffer[1], clip);
    assert_eq!(buffer[2], -clip);
    assert_eq!(buffer[3], 0.25);

    let mut nan_gain = [0.5];
    let clipped = Output::apply_volume(nan_gain.iter_mut(), std::iter::once(Sample::NAN), 1);

    assert!(clipped);
    assert_eq!(nan_gain[0], 0.0);
}

#[test]
fn read_output_copies_each_channel() {
    let mut output = Output::new(StereoSample::ONE, 0.0);

    for (channel, buffer) in output.output.iter_mut().enumerate() {
        for (index, sample) in buffer.iter_mut().enumerate() {
            *sample = channel as Sample + index as Sample * 0.01;
        }
    }

    let mut left = vec![-1.0; 5];
    let mut right = vec![-1.0; 3];
    output.read_output(false, &mut [&mut left[..], &mut right[..]]);

    assert_eq!(left, [0.0, 0.01, 0.02, 0.03, 0.04]);
    assert_eq!(right, [1.0, 1.01, 1.02]);
}

#[test]
fn read_output_decimates_when_oversampling() {
    let mut output = Output::new(StereoSample::ONE, 0.0);
    let mut oracle = IirDecimator::new();

    for sample in &mut output.output[0][..8] {
        *sample = 0.5;
    }
    for sample in &mut output.output[1][..8] {
        *sample = -0.25;
    }

    let mut expected_left = vec![0.0; 4];
    let mut expected_right = vec![0.0; 4];
    oracle.process(
        [&output.output[0], &output.output[1]],
        [&mut expected_left, &mut expected_right],
    );

    let mut left = vec![-1.0; 130];
    let mut right = vec![-1.0; 130];
    output.read_output(true, &mut [&mut left[..], &mut right[..]]);

    assert_eq!(&left[..4], expected_left.as_slice());
    assert_eq!(&right[..4], expected_right.as_slice());
    assert!(left[..4].iter().all(|sample| sample.is_finite()));
    assert!(left[128..].iter().all(|sample| *sample == -1.0));
    assert!(right[128..].iter().all(|sample| *sample == -1.0));
}

#[test]
fn trigger_stage_leaves_output_and_meters_untouched() {
    let mut output = Output::new(StereoSample::ONE, 0.0);
    output.output[0][0] = Sample::NAN;
    output.output[1][BUFFER_SIZE - 1] = 3.0;

    let meter = run_process(&mut output, 16, true, true);

    assert!(output.output[0][0].is_nan());
    assert_eq!(output.output[1][BUFFER_SIZE - 1], 3.0);
    assert_eq!(meter.volume, StereoSample::ZERO);
    assert_eq!(meter.clipped, [false, false]);
}

#[test]
fn process_with_no_voices_clears_output_and_publishes_silence() {
    let mut output = Output::new(StereoSample::splat(2.0), 0.0);
    output.output[0].fill(0.7);
    output.output[1].fill(-0.4);

    let meter = run_process(&mut output, 32, false, true);

    assert!(
        output
            .output
            .iter()
            .all(|buffer| buffer.iter().all(|sample| *sample == 0.0))
    );
    assert_eq!(meter.volume, StereoSample::ZERO);
    assert_eq!(meter.clipped, [false, false]);
    assert_eq!(output.get_gain(), StereoSample::splat(2.0));
}

#[test]
fn unconnected_output_stays_silent_while_a_note_plays() {
    let mut engine = make_engine(
        EngineParams {
            output_gain: StereoSample::ONE,
            ..EngineParams::default()
        },
        false,
    );
    let (left, right) = play(&mut engine, 60);

    assert!(left.iter().chain(right.iter()).all(|sample| *sample == 0.0));
}

#[test]
fn output_gain_scales_each_channel() {
    let params = |gain: StereoSample| EngineParams {
        output_gain: gain,
        ..EngineParams::default()
    };
    let mut unity = make_engine(params(StereoSample::ONE), true);
    let mut scaled = make_engine(params(StereoSample::new(1.0, 0.5)), true);

    unity.handle_note_on(note(60), 0);
    scaled.handle_note_on(note(60), 0);

    let (left, right) = process_block(&mut unity, 64);
    let (scaled_left, scaled_right) = process_block(&mut scaled, 64);

    assert!(left.iter().any(|sample| sample.abs() > 1e-3));
    for (scaled, unity) in scaled_left.iter().zip(&left) {
        assert_close(*scaled, *unity);
    }
    for (scaled, unity) in scaled_right.iter().zip(&right) {
        assert_close(*scaled, *unity * 0.5);
    }
}

#[test]
fn output_gain_ramps_instead_of_jumping() {
    let mut steady = make_engine(
        EngineParams {
            output_gain: StereoSample::ONE,
            ..EngineParams::default()
        },
        true,
    );
    let mut ramped = make_engine(
        EngineParams {
            output_gain: StereoSample::ONE,
            ..EngineParams::default()
        },
        true,
    );

    steady.handle_note_on(note(60), 0);
    ramped.handle_note_on(note(60), 0);
    ramped.set_output_gain(StereoSample::splat(2.0));

    let (steady_left, _) = process_block(&mut steady, 64);
    let (ramped_left, _) = process_block(&mut ramped, 64);
    let steady_peak = peak(&steady_left);
    let ramped_peak = peak(&ramped_left);

    assert!(ramped_peak > steady_peak);
    assert!(ramped_peak < steady_peak * 1.5);
}

#[test]
fn voices_are_summed_before_the_output_gain() {
    let params = EngineParams {
        num_voices: 2,
        output_gain: StereoSample::splat(0.05),
        ..EngineParams::default()
    };
    let mut both = make_engine(params.clone(), true);
    let mut low = make_engine(params.clone(), true);
    let mut high = make_engine(params, true);

    both.handle_note_on(note(60), 0);
    both.handle_note_on(note(67), 0);
    low.handle_note_on(note(60), 0);
    high.handle_note_on(note(67), 0);

    let (both_left, both_right) = process_block(&mut both, 64);
    let (low_left, low_right) = process_block(&mut low, 64);
    let (high_left, high_right) = process_block(&mut high, 64);

    for ((summed, low), high) in both_left.iter().zip(&low_left).zip(&high_left) {
        assert_close(*summed, *low + *high);
    }
    for ((summed, low), high) in both_right.iter().zip(&low_right).zip(&high_right) {
        assert_close(*summed, *low + *high);
    }
}

#[test]
fn output_is_hard_clipped_at_12_db() {
    let clip = db_to_gain_fast(super::OUTPUT_CLIP_DB);
    let mut engine = make_engine(
        EngineParams {
            output_gain: StereoSample::splat(4.0),
            ..EngineParams::default()
        },
        true,
    );

    engine.handle_note_on(note(60), 0);

    let mut rendered = Vec::new();
    for _ in 0..8 {
        let (left, right) = process_block(&mut engine, 128);
        rendered.extend(left);
        rendered.extend(right);
    }

    let rendered_peak = peak(&rendered);
    assert!(
        rendered
            .iter()
            .all(|sample| sample.is_finite() && sample.abs() <= clip + 1e-5),
        "peak {rendered_peak} exceeds {clip}"
    );
    assert!(
        rendered_peak > clip * 0.99,
        "expected the 12 dB ceiling, peak {rendered_peak} clip {clip}"
    );
}

fn peak(samples: &[Sample]) -> Sample {
    samples
        .iter()
        .fold(0.0, |peak, sample| peak.max(sample.abs()))
}
