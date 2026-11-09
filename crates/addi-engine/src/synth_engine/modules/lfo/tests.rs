use addi_dsp::segments::rel_segments::{RelSegments, SegPoint};

use super::*;
use crate::synth_engine::{
    CONTROL_EXT, HostTimeline, Note, SmoothedSampleParams, VoiceEvent,
    routing::{OutputsArena, ProcessContext, ProcessParams},
    stub::AudioEnd,
    synth_module::SynthModule,
    voices_handler::{PlayingVoice, PlayingVoices, VoicesHandler},
};

type TestLfo = Lfo<stub::Links>;

fn assert_points_eq(actual: &[SegPoint], expected: &[SegPoint]) {
    assert_eq!(actual.len(), expected.len());
    for (a, e) in actual.iter().zip(expected) {
        assert_eq!(a.x, e.x);
        assert_eq!(a.y, e.y);
        assert_eq!(a.curve, e.curve);
    }
}

fn test_square() -> RelSegments {
    RelSegments::from_points(
        &[(0.0, 0.0), (0.0, 1.0), (0.5, 1.0), (0.5, 0.0), (1.0, 0.0)],
        false,
    )
}

fn test_sine() -> RelSegments {
    let mut segments = RelSegments::from_points(&[(0.0, 0.0), (0.5, 1.0), (1.0, 0.0)], false);

    for index in 0..segments.num_points() {
        segments.set_smooth(index, true);
    }

    segments
}

fn make_lfo(segments: RelSegments) -> TestLfo {
    Lfo::from_config(
        &LfoConfig {
            id: 1,
            segments,
            ..LfoConfig::default()
        },
        16,
    )
}

fn phase_at(index: usize, fraction: Smp) -> Phase {
    let shift = 32 - WAVEFORM_BITS;
    let bits = ((index as u32) << shift).wrapping_add((fraction * (1u32 << shift) as Smp) as u32);

    Phase::from_bits(bits)
}

#[test]
fn table_knots_match_the_segments() {
    let lfo = make_lfo(test_sine());

    let mut rendered = vec![0.0; WAVEFORM_SIZE];
    lfo.get_config().segments.render_waveform(&mut rendered);

    for index in [0, 1, 100, 1024, 2047] {
        let baked = lfo.wave[WAVEFORM_PAD_LEFT + index];

        assert_eq!(baked, rendered[index]);
        assert_eq!(
            TestLfo::read_wave(&lfo.wave, phase_at(index, 0.0)),
            baked,
            "index {index}"
        );
    }
}

#[test]
fn sine_midpoints_follow_catmull_rom() {
    let lfo = make_lfo(test_sine());

    let mut fine = vec![0.0; WAVEFORM_SIZE * 2];
    lfo.get_config().segments.render_waveform(&mut fine);

    for index in [0, 10, 500, 2046] {
        let got = TestLfo::read_wave(&lfo.wave, phase_at(index, 0.5));
        let expected = fine[index * 2 + 1];

        assert!(
            (got - expected).abs() < 1e-4,
            "index {index}: {got} vs {expected}"
        );
    }
}

#[test]
fn playback_applies_phase_shift() {
    let lfo = make_lfo(test_sine());
    let index = 0;
    let shift = 0.125;
    let shift_index = (shift * WAVEFORM_SIZE as Smp) as usize;
    let read = TestLfo::read_wave(&lfo.wave, phase_at(index, 0.0).add_normalized(shift));
    let expected = lfo.wave[WAVEFORM_PAD_LEFT + shift_index];

    assert_eq!(read, expected);
}

#[test]
fn set_segments_rebakes_the_table() {
    let mut lfo = make_lfo(RelSegments::default());
    let before = lfo.wave[WAVEFORM_PAD_LEFT + 100];

    lfo.set_segments(test_square());

    assert_ne!(lfo.wave[WAVEFORM_PAD_LEFT + 100], before);
    assert_points_eq(lfo.get_config().segments.points(), test_square().points());
    assert_eq!(lfo.wave[WAVEFORM_PAD_LEFT + 100], 1.0);
    assert_eq!(lfo.wave[WAVEFORM_PAD_LEFT + 1500], 0.0);

    lfo.set_segments(test_square());

    assert_points_eq(lfo.get_config().segments.points(), test_square().points());
}

#[test]
fn from_config_sanitizes_params() {
    let lfo = TestLfo::from_config(
        &LfoConfig {
            rate: StereoSample::new(1_000.0, Smp::NAN),
            phase_shift: StereoSample::new(-4.0, 0.25),
            segments: RelSegments::from_points(&[(0.0, -2.0), (1.0, 3.0)], false),
            ..LfoConfig::default()
        },
        16,
    );
    let config = lfo.get_config();

    assert_eq!(config.rate, StereoSample::new(100.0, 0.0));
    assert_eq!(config.phase_shift, StereoSample::new(-1.0, 0.25));
    assert_eq!(config.segments.point(0).y, 0.0);
    assert_eq!(config.segments.point(1).y, 1.0);
}

#[test]
fn missing_timebase_defaults_to_hertz() {
    let config: LfoConfig = serde_json::from_str(r#"{"id":1}"#).unwrap();

    assert_eq!(config.timebase, Timebase::Hertz);
    assert_eq!(config.rate, StereoSample::splat(1.0));
    assert!(!config.sync);

    let legacy: LfoConfig =
        serde_json::from_str(r#"{"id":1,"frequency":{"channels":[2.0,3.0]}}"#).unwrap();

    assert_eq!(legacy.rate, StereoSample::new(2.0, 3.0));
}

#[test]
fn sync_round_trips() {
    let stored = LfoConfig {
        id: 1,
        sync: true,
        ..LfoConfig::default()
    };
    let loaded: LfoConfig = serde_json::from_str(&serde_json::to_string(&stored).unwrap()).unwrap();

    assert!(loaded.sync);
    assert_eq!(loaded.timebase, Timebase::Hertz);
}

#[test]
fn wrapped_read_stays_finite() {
    let lfo = make_lfo(test_square());
    let y = TestLfo::read_wave(&lfo.wave, Phase::from_bits(u32::MAX));

    assert!(y.is_finite());
}

const SAMPLE_RATE: Smp = 48_000.0;

fn quarter_lfo(sync: bool, steal_phase: bool) -> TestLfo {
    Lfo::from_config(
        &LfoConfig {
            id: 1,
            sync,
            steal_phase,
            timebase: Timebase::Quarter,
            ..LfoConfig::default()
        },
        128,
    )
}

fn note(midi: u8) -> Note {
    Note {
        channel: 0,
        note: midi,
        velocity: 1.0,
        host_id: None,
    }
}

fn triggered_at(index: usize, offset: usize) -> PlayingVoice {
    let mut handler = VoicesHandler::new(8, false, false);

    for i in 0..=index {
        handler.handle_note_on(note(60 + i as u8), offset);
    }

    let mut playing = PlayingVoices::new();
    handler.get_playing_voices(&mut playing);

    playing
        .into_iter()
        .find(|voice| voice.voice_idx() == index)
        .expect("allocated voice")
}

fn steps(block: usize, triggered: Option<usize>) -> usize {
    block - triggered.unwrap_or(1) + CONTROL_EXT
}

fn advance(mut phase: Phase, count: usize) -> Phase {
    let mut rates = [1.0];

    Timebase::Quarter.map_phase_inc(&mut rates, 120.0, Phase::freq_phase_mult(SAMPLE_RATE));

    for _ in 0..count {
        phase += rates[0];
    }

    phase
}

fn timeline(beats: f64) -> HostTimeline {
    HostTimeline::new(f64::from(SAMPLE_RATE), 120.0, beats * 0.5, beats)
}

struct Rig {
    arena: OutputsArena,
    audio_end: AudioEnd,
}

impl Rig {
    fn new(lfo: &mut TestLfo) -> Self {
        let mut arena = OutputsArena::new(128);

        arena.allocate_slot(lfo);

        Self {
            arena,
            audio_end: AudioEnd,
        }
    }

    fn process(
        &mut self,
        lfo: &mut TestLfo,
        voice: PlayingVoice,
        beats: f64,
        block: usize,
        trigger_stage: bool,
    ) {
        let has_triggered_voices = voice.triggered().is_some();
        let last_voice_idx = Some(voice.voice_idx());
        let voices = [voice];
        let timeline = timeline(beats);
        let mut ctx = ProcessContext {
            outputs_arena: &mut self.arena,
            audio_end: &mut self.audio_end,
            params: ProcessParams {
                trigger_stage,
                has_triggered_voices,
                samples: block,
                sample_rate: SAMPLE_RATE,
                timeline,
                needs_update_ui: false,
                smooth: SmoothedSampleParams::new(SAMPLE_RATE),
                active_voices: &voices,
                last_voice_idx,
            },
        };

        lfo.process(&mut ctx);
    }
}

fn voice_phases(lfo: &TestLfo, voice_idx: usize) -> [Phase; NUM_CHANNELS] {
    std::array::from_fn(|channel| lfo.voices.at(channel, voice_idx).phase)
}

#[test]
fn sync_starts_a_triggered_voice_on_the_transport() {
    let mut lfo = quarter_lfo(true, false);
    let mut rig = Rig::new(&mut lfo);
    let block = 16;

    rig.process(&mut lfo, triggered_at(0, 0), 0.25, block, true);

    let initial = Timebase::Quarter.transport_phase(1.0, &timeline(0.25), 0);
    let expected = advance(initial, steps(block, Some(0)));

    assert_ne!(initial, Phase::ZERO);
    assert_eq!(voice_phases(&lfo, 0), [expected; NUM_CHANNELS]);
}

#[test]
fn sync_leaves_a_running_voice_free() {
    let mut lfo = quarter_lfo(true, false);
    let mut rig = Rig::new(&mut lfo);
    let block = 16;

    rig.process(&mut lfo, PlayingVoice::new(0, 60), 0.25, block, false);

    let expected = advance(Phase::ZERO, steps(block, None));

    assert_eq!(voice_phases(&lfo, 0), [expected; NUM_CHANNELS]);
}

#[test]
fn sync_uses_the_trigger_offset() {
    let mut lfo = quarter_lfo(true, false);
    let mut rig = Rig::new(&mut lfo);
    let offset = 48;
    let block = 64;

    rig.process(&mut lfo, triggered_at(0, offset), 0.0, block, true);

    let initial = Timebase::Quarter.transport_phase(1.0, &timeline(0.0), offset);
    let expected = advance(initial, steps(block, Some(offset)));

    assert_ne!(initial, Phase::ZERO);
    assert_eq!(voice_phases(&lfo, 0), [expected; NUM_CHANNELS]);
}

#[test]
fn steal_phase_wins_over_sync() {
    let mut lfo = quarter_lfo(true, true);
    let mut rig = Rig::new(&mut lfo);
    let block = 16;

    rig.process(&mut lfo, PlayingVoice::new(0, 60), 0.5, block, false);

    let stolen = voice_phases(&lfo, 0)[0];
    let transport = Timebase::Quarter.transport_phase(1.0, &timeline(0.5), 0);

    assert_ne!(stolen, transport);

    SynthModule::process_events(
        &mut lfo,
        &[VoiceEvent::Reset {
            voice_idx: 1,
            replaced_voice_idx: Some(0),
            prev_note: None,
            pitch: 0.0,
            velocity: 1.0,
            offset: 0,
        }],
    );

    rig.process(&mut lfo, triggered_at(1, 0), 0.5, block, true);

    let expected = advance(stolen, steps(block, Some(0)));

    assert_eq!(voice_phases(&lfo, 1), [expected; NUM_CHANNELS]);
}
