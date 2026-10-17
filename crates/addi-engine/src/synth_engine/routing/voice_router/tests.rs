use std::array;

use addi_dsp::SmoothedSample;

use super::{OutputRouterType, OutputVoiceState, VoiceLane, VoiceOutput, VoicesBlock};
use crate::synth_engine::{
    CONTROL_EXT, ComplexSample, DataType, EngineAudioEnd, Input, ModuleId, NUM_CHANNELS,
    SPECTRAL_BUFFER_SIZE, SmoothedSampleParams, Smp, StereoSample,
    buffer::{VoicesLayout, zero_buffer},
    engine_io::{UiEvent, VoicesTelemetry},
    routing::{
        InputSlot, MixedSlots, OutputsArena, ProcessContext, ProcessParams, SamplesOutput,
        SpectralOutput, VoiceTarget,
    },
    synth_module::SynthModule,
    voices::{VOICE_BLOCK_LEN, VOICE_BLOCKS},
    voices_handler::{Note, PlayingVoice, PlayingVoices, VoicesHandler},
};

const SAMPLE_RATE: Smp = 48_000.0;
const BUFFER_SIZE: usize = 32;
const SAMPLES: usize = 8;
const MODULE_ID: ModuleId = 7;

#[derive(Clone, Debug)]
struct ModUpdate {
    module_id: ModuleId,
    input: Input,
    channel: u8,
    value: Smp,
    normalized: Smp,
}

#[derive(Default)]
struct RecordingEnd {
    updates: Vec<ModUpdate>,
}

impl EngineAudioEnd for RecordingEnd {
    fn update_modulated_input(
        &mut self,
        module_id: ModuleId,
        input: Input,
        channel: u8,
        value: Smp,
        normalized_value: Smp,
    ) -> bool {
        self.updates.push(ModUpdate {
            module_id,
            input,
            channel,
            value,
            normalized: normalized_value,
        });
        true
    }

    fn update_voices_status(&mut self, _telemetry: &VoicesTelemetry) -> bool {
        false
    }

    fn pop_event(&mut self) -> Option<UiEvent> {
        None
    }

    fn update_out_volume(&mut self, _volume: StereoSample, _clipped: [bool; NUM_CHANNELS]) {}
}

struct SlotModule {
    kind: DataType,
    slot: usize,
}

impl SynthModule for SlotModule {
    fn id(&self) -> ModuleId {
        MODULE_ID
    }

    fn inputs(&self) -> &[crate::synth_engine::routing::InputMeta] {
        &[]
    }

    fn output_type(&self) -> DataType {
        self.kind
    }

    fn set_output_slot(&mut self, slot: usize) {
        self.slot = slot;
    }

    fn output_slot(&self) -> usize {
        self.slot
    }

    fn process_ui_events(&mut self) {}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Visit {
    voice: usize,
    channel: usize,
    triggered: Option<usize>,
}

struct LaneTrace {
    prepares: Vec<(usize, usize, usize)>,
    blocks: Vec<VoicesBlock>,
}

struct Rig {
    arena: OutputsArena,
    end: RecordingEnd,
    voices: Vec<PlayingVoice>,
    trigger_stage: bool,
    needs_update_ui: bool,
    last_voice_idx: Option<usize>,
}

impl Rig {
    fn new(voices: Vec<PlayingVoice>) -> Self {
        let last_voice_idx = voices.last().map(|voice| voice.voice_idx());

        Self {
            arena: OutputsArena::new(BUFFER_SIZE),
            end: RecordingEnd::default(),
            voices,
            trigger_stage: false,
            needs_update_ui: false,
            last_voice_idx,
        }
    }

    fn alloc(&mut self, kind: DataType) -> usize {
        let mut module = SlotModule { kind, slot: 0 };
        self.arena.allocate_slot(&mut module);
        module.slot
    }

    fn alloc_audio(&mut self) -> usize {
        self.alloc(DataType::Audio)
    }

    fn alloc_control(&mut self) -> usize {
        self.alloc(DataType::Control)
    }

    fn alloc_spectral(&mut self) -> usize {
        self.alloc(DataType::Spectral)
    }

    fn free(&mut self, slot: usize, kind: DataType) {
        self.arena.free_slot(&SlotModule { kind, slot });
    }

    fn context(&mut self) -> ProcessContext<'_, RecordingEnd> {
        ProcessContext {
            outputs_arena: &mut self.arena,
            audio_end: &mut self.end,
            params: ProcessParams {
                trigger_stage: self.trigger_stage,
                has_triggered_voices: self.voices.iter().any(|voice| voice.triggered().is_some()),
                samples: SAMPLES,
                sample_rate: SAMPLE_RATE,
                needs_update_ui: self.needs_update_ui,
                smooth_params: SmoothedSampleParams::new(SAMPLE_RATE),
                active_voices: &self.voices,
                last_voice_idx: self.last_voice_idx,
            },
        }
    }

    fn with_audio_slot<R>(
        &mut self,
        slot: usize,
        f: impl FnOnce(&mut ProcessContext<'_, RecordingEnd>, &mut VoicesLayout<SamplesOutput>) -> R,
    ) -> R {
        let mut outputs = self.arena.samples[slot].slot.take().expect("audio slot");
        let result = {
            let mut ctx = self.context();
            f(&mut ctx, &mut outputs)
        };
        self.arena.samples[slot].slot = Some(outputs);
        result
    }

    fn with_control_slot<R>(
        &mut self,
        slot: usize,
        f: impl FnOnce(&mut ProcessContext<'_, RecordingEnd>, &mut VoicesLayout<SamplesOutput>) -> R,
    ) -> R {
        let mut outputs = self.arena.samples[slot].slot.take().expect("control slot");
        let result = {
            let mut ctx = self.context();
            f(&mut ctx, &mut outputs)
        };
        self.arena.samples[slot].slot = Some(outputs);
        result
    }

    fn with_spectral_slot<R>(
        &mut self,
        slot: usize,
        f: impl FnOnce(&mut ProcessContext<'_, RecordingEnd>, &mut VoicesLayout<SpectralOutput>) -> R,
    ) -> R {
        let mut outputs = self.arena.spectral[slot]
            .slot
            .take()
            .expect("spectral slot");
        let result = {
            let mut ctx = self.context();
            f(&mut ctx, &mut outputs)
        };
        self.arena.spectral[slot].slot = Some(outputs);
        result
    }

    fn sample_buf(&mut self, slot: usize, channel: usize, voice: usize) -> &mut [Smp] {
        self.arena.samples[slot].at_mut(channel, voice).buffer_mut()
    }

    fn set_next(&mut self, slot: usize, channel: usize, voice: usize, value: Smp) {
        self.arena.samples[slot]
            .at_mut(channel, voice)
            .next_frame_sample = value;
    }

    fn next(&self, slot: usize, channel: usize, voice: usize) -> Smp {
        self.arena.samples[slot]
            .at(channel, voice)
            .next_frame_sample
    }

    fn fill_samples(&mut self, slot: usize, channel: usize, voice: usize, value: Smp) {
        self.sample_buf(slot, channel, voice).fill(value);
    }

    fn set_spectrum(&mut self, slot: usize, channel: usize, voice: usize, bins: &[ComplexSample]) {
        let output = self.arena.spectral[slot].at_mut(channel, voice);
        output.set_length(bins.len());
        output.get_mut().copy_from_slice(bins);
    }
}

fn midi(note: u8) -> Note {
    Note {
        channel: 0,
        note,
        velocity: 1.0,
        host_id: None,
    }
}

fn note_on_voice(offset: usize) -> PlayingVoice {
    let mut handler = VoicesHandler::new(4, false, false);
    handler.handle_note_on(midi(60), offset);

    let mut playing = PlayingVoices::new();
    handler.get_playing_voices(&mut playing);
    let voice = playing[0];

    assert_eq!(voice.voice_idx(), 0);
    assert_eq!(voice.triggered(), Some(offset));
    voice
}

fn target(channel: usize, voice: usize, triggered: Option<usize>, is_last: bool) -> VoiceTarget {
    VoiceTarget {
        channel_idx: channel,
        voice_idx: voice,
        note: 60,
        triggered,
        is_last,
    }
}

fn steady_voices() -> Vec<PlayingVoice> {
    vec![PlayingVoice::new(0, 60), PlayingVoice::new(1, 64)]
}

fn mixed_voices() -> Vec<PlayingVoice> {
    vec![note_on_voice(4), PlayingVoice::new(1, 64)]
}

fn visit(target: &VoiceTarget) -> Visit {
    Visit {
        voice: target.voice_idx,
        channel: target.channel_idx,
        triggered: target.triggered,
    }
}

fn source(slot: usize, amount: StereoSample, modulation: Option<usize>) -> InputSlot {
    InputSlot {
        src_slot: slot,
        modulation_slot: modulation,
        amount,
    }
}

fn mixed(sources: impl IntoIterator<Item = InputSlot>) -> MixedSlots {
    let mut input = MixedSlots::new(Input::Cutoff);
    input.slots.extend(sources);
    input
}

fn approx(actual: Smp, expected: Smp) {
    assert!((actual - expected).abs() <= 1e-5, "{actual} ≈ {expected}");
}

fn assert_update(update: &ModUpdate, channel: u8, value: Smp, normalized: Smp) {
    assert_eq!(update.module_id, MODULE_ID);
    assert_eq!(update.input, Input::Cutoff);
    assert_eq!(update.channel, channel);
    assert_eq!(update.value, value);
    approx(update.normalized, normalized);
}

#[test]
fn lane_clears_samples_before_trigger() {
    let lane = VoiceLane {
        lane_idx: 2,
        offset: 3,
        samples: 4,
    };
    let mut buffers: [_; 4] = array::from_fn(|_| zero_buffer(16));

    buffers[2].fill(1.0);

    let active = lane.buff(&mut buffers);
    let active = active.to_vec();

    assert_eq!(active, [1.0; 4]);
    assert_eq!(&buffers[2][..3], &[0.0; 3]);
    assert_eq!(buffers[2][3], 1.0);
    assert_eq!(buffers[2][7], 1.0);
    assert_eq!(buffers[1][0], 0.0);
    assert_eq!(buffers[2].len(), 16);
}

#[test]
fn params_expose_the_process_context() {
    let mut rig = Rig::new(vec![PlayingVoice::new(0, 60)]);
    let slot = rig.alloc_audio();
    let mut ctx = rig.context();
    let mut factory = ctx.audio(MODULE_ID, slot);

    assert_eq!(factory.params().sample_rate, SAMPLE_RATE);
    assert_eq!(factory.params().samples, SAMPLES);
    assert!(!factory.params().trigger_stage);

    let voice_target = target(0, 0, None, false);
    let voice = factory.router(&voice_target);
    assert_eq!(voice.sample_rate(), SAMPLE_RATE);
}

#[test]
fn for_channels_runs_both_channels_outside_the_trigger_stage() {
    let mut rig = Rig::new(vec![]);
    let slot = rig.alloc_audio();
    let mut seen = Vec::new();
    let mut ctx = rig.context();

    ctx.audio(MODULE_ID, slot).for_channels(|factory, channel| {
        assert_eq!(factory.params().samples, SAMPLES);
        seen.push(channel);
    });

    assert_eq!(seen, [0, 1]);
}

#[test]
fn iteration_skips_audio_work_during_the_trigger_stage() {
    let mut rig = Rig::new(steady_voices());
    rig.trigger_stage = true;
    let slot = rig.alloc_audio();
    rig.free(slot, DataType::Audio);

    let mut voices = 0;
    let mut lanes = 0;
    let mut channels = 0;
    let mut ctx = rig.context();
    let mut factory = ctx.audio(MODULE_ID, slot);

    factory.for_voices(|_, _, _| voices += 1);
    factory.for_lanes(
        &mut lanes,
        |lanes, _, _, _| *lanes += 1,
        |lanes, _, _, _| *lanes += 1,
    );
    factory.for_channels(|_, _| channels += 1);

    assert_eq!((voices, lanes, channels), (0, 0, 0));
}

#[test]
fn audio_for_voices_visits_untriggered_voices_before_triggered_ones() {
    let mut rig = Rig::new(mixed_voices());
    let slot = rig.alloc_audio();
    rig.fill_samples(slot, 0, 0, 3.5);

    let mut visits = Vec::new();
    let mut ctx = rig.context();
    ctx.audio(MODULE_ID, slot).for_voices(|factory, voice, _| {
        assert!(factory.ctx.outputs_arena.samples[slot].slot.is_none());
        visits.push(visit(voice));
    });

    assert_eq!(
        visits,
        [
            Visit {
                voice: 1,
                channel: 0,
                triggered: None,
            },
            Visit {
                voice: 1,
                channel: 1,
                triggered: None,
            },
            Visit {
                voice: 0,
                channel: 0,
                triggered: Some(4),
            },
            Visit {
                voice: 0,
                channel: 1,
                triggered: Some(4),
            },
        ]
    );
    assert_eq!(rig.arena.samples[slot].at(0, 0).buffer()[0], 3.5);
}

#[test]
fn audio_for_voices_with_no_voices_restores_the_slot() {
    let mut rig = Rig::new(vec![]);
    let slot = rig.alloc_audio();
    rig.fill_samples(slot, 0, 0, 2.0);

    let mut visits = 0;
    let mut ctx = rig.context();
    ctx.audio(MODULE_ID, slot)
        .for_voices(|_, _, _| visits += 1)
        .for_channels(|_, _| {});

    assert_eq!(visits, 0);
    assert_eq!(rig.arena.samples[slot].at(0, 0).buffer()[0], 2.0);
}

#[test]
#[should_panic(expected = "slot should be in place")]
fn audio_for_voices_panics_when_the_slot_is_freed() {
    let mut rig = Rig::new(vec![PlayingVoice::new(0, 60)]);
    let slot = rig.alloc_audio();
    rig.free(slot, DataType::Audio);

    let mut ctx = rig.context();
    ctx.audio(MODULE_ID, slot).for_voices(|_, _, _| {});
}

#[test]
fn for_lanes_keeps_each_voice_in_its_pair_slot() {
    let mut rig = Rig::new(vec![PlayingVoice::new(1, 64), PlayingVoice::new(0, 60)]);
    rig.last_voice_idx = Some(0);
    rig.needs_update_ui = true;
    let slot = rig.alloc_audio();
    rig.fill_samples(slot, 0, 0, 6.0);

    let mut trace = LaneTrace {
        prepares: Vec::new(),
        blocks: Vec::new(),
    };
    let mut ctx = rig.context();
    ctx.audio(MODULE_ID, slot).for_lanes(
        &mut trace,
        |trace, factory, voice, lane| {
            let router = factory.router(voice);
            assert_eq!(router.triggered(), voice.triggered.is_some());
            assert_eq!(router.samples(), SAMPLES);
            assert_eq!(router.need_update_ui(), voice.voice_idx == 0);
            assert_eq!(
                router.need_update_ui_mono(),
                voice.voice_idx == 0 && voice.channel_idx == 0
            );
            trace
                .prepares
                .push((voice.voice_idx, voice.channel_idx, lane));
        },
        |trace, factory, block, outputs| {
            assert!(factory.ctx.outputs_arena.samples[slot].slot.is_none());
            assert_eq!(outputs.at(0, 0).buffer()[0], 6.0);
            trace.blocks.push(*block);
        },
    );

    assert_eq!(
        trace.prepares,
        [
            (0, 0, 0),
            (0, 1, 1),
            (1, 0, NUM_CHANNELS),
            (1, 1, NUM_CHANNELS + 1),
        ]
    );
    assert_eq!(trace.blocks.len(), 1);
    let block = &trace.blocks[0];
    assert_eq!(block.block_idx, 0);
    assert_eq!(block.last_voice_idx, Some(0));
    assert_eq!(block.voices[0].map(|voice| voice.voice_idx()), Some(0));
    assert_eq!(block.voices[1].map(|voice| voice.voice_idx()), Some(1));
    assert_eq!(rig.arena.samples[slot].at(0, 0).buffer()[0], 6.0);
}

#[test]
fn for_lanes_skips_empty_pairs_and_empty_slots() {
    let next_block = VOICE_BLOCK_LEN;
    let mut rig = Rig::new(vec![
        PlayingVoice::new(1, 62),
        PlayingVoice::new(next_block as u8, 67),
    ]);
    let slot = rig.alloc_audio();
    let mut trace = LaneTrace {
        prepares: Vec::new(),
        blocks: Vec::new(),
    };
    let mut ctx = rig.context();
    ctx.audio(MODULE_ID, slot).for_lanes(
        &mut trace,
        |trace, _, voice, lane| {
            trace
                .prepares
                .push((voice.voice_idx, voice.channel_idx, lane));
        },
        |trace, _, block, _| trace.blocks.push(*block),
    );

    assert_eq!(
        trace.prepares,
        [
            (1, 0, NUM_CHANNELS),
            (1, 1, NUM_CHANNELS + 1),
            (next_block, 0, 0),
            (next_block, 1, 1),
        ]
    );
    assert_eq!(trace.blocks.len(), 2);
    assert_eq!(trace.blocks[0].block_idx, 0);
    assert!(trace.blocks[0].voices[0].is_none());
    assert_eq!(
        trace.blocks[0].voices[1].map(|voice| voice.voice_idx()),
        Some(1)
    );
    assert!(
        trace.blocks[0].voices[2..]
            .iter()
            .all(|voice| voice.is_none())
    );
    assert_eq!(trace.blocks[1].block_idx, 1);
    assert_eq!(
        trace.blocks[1].voices[0].map(|voice| voice.voice_idx()),
        Some(next_block)
    );
    assert!(
        trace.blocks[1].voices[1..]
            .iter()
            .all(|voice| voice.is_none())
    );
    assert!(trace.blocks.len() < VOICE_BLOCKS);
}

#[test]
#[should_panic(expected = "slot should be in place")]
fn audio_for_lanes_panics_when_the_slot_is_freed() {
    let mut rig = Rig::new(vec![PlayingVoice::new(0, 60)]);
    let slot = rig.alloc_audio();
    rig.free(slot, DataType::Audio);

    let mut ctx = rig.context();
    ctx.audio(MODULE_ID, slot)
        .for_lanes(&mut (), |_, _, _, _| {}, |_, _, _, _| {});
}

#[test]
fn trigger_stage_visits_every_active_voice() {
    let mut rig = Rig::new(mixed_voices());
    rig.trigger_stage = true;
    let slot = rig.alloc_audio();
    let mut visits = Vec::new();
    let mut ctx = rig.context();

    ctx.audio(MODULE_ID, slot)
        .for_triggered_voices(|_, voice| visits.push(visit(voice)));

    assert_eq!(
        visits,
        [
            Visit {
                voice: 0,
                channel: 0,
                triggered: Some(4),
            },
            Visit {
                voice: 0,
                channel: 1,
                triggered: Some(4),
            },
            Visit {
                voice: 1,
                channel: 0,
                triggered: None,
            },
            Visit {
                voice: 1,
                channel: 1,
                triggered: None,
            },
        ]
    );
}

#[test]
fn process_stage_skips_triggered_voice_iteration() {
    let mut rig = Rig::new(mixed_voices());
    let slot = rig.alloc_audio();
    let mut visits = 0;
    let mut ctx = rig.context();

    ctx.audio(MODULE_ID, slot)
        .for_triggered_voices(|_, _| visits += 1);

    assert_eq!(visits, 0);
}

#[test]
fn router_and_for_triggered_voice_leave_audio_in_place() {
    let mut rig = Rig::new(vec![]);
    let slot = rig.alloc_audio();
    rig.fill_samples(slot, 0, 0, 1.0);
    let voice = target(0, 0, Some(3), false);

    rig.with_audio_slot(slot, |ctx, _outputs| {
        let mut factory = ctx.audio(MODULE_ID, slot);
        {
            let router = factory.router(&voice);
            assert!(router.triggered());
            assert_eq!(router.samples(), SAMPLES - 3);
        }

        let again = factory.for_triggered_voice(&voice);
        assert!(again.triggered());
        assert_eq!(again.samples(), SAMPLES - 3);
    });

    assert_eq!(rig.arena.samples[slot].at(0, 0).buffer()[0], 1.0);
}

#[test]
fn for_voice_clears_audio_before_the_trigger() {
    let mut rig = Rig::new(vec![]);
    let slot = rig.alloc_audio();
    rig.fill_samples(slot, 0, 0, 4.0);
    rig.set_next(slot, 0, 0, 5.0);
    let voice = target(0, 0, Some(3), true);

    rig.with_audio_slot(slot, |ctx, outputs| {
        let mut factory = ctx.audio(MODULE_ID, slot);
        let (router, mut rendered) = factory.for_voice(&voice, outputs);

        assert!(router.triggered());
        assert_eq!(router.samples(), SAMPLES - 3);
        assert_eq!(rendered.output().len(), SAMPLES - 3);
        rendered.output().fill(0.25);

        let mut buffers = [zero_buffer(BUFFER_SIZE)];
        let active = router.lane(0).buff(&mut buffers);
        assert_eq!(active.len(), SAMPLES - 3);
        assert_eq!(&buffers[0][..3], &[0.0; 3]);
    });

    let buffer = rig.arena.samples[slot].at(0, 0).buffer();
    assert_eq!(&buffer[..3], &[0.0; 3]);
    assert_eq!(&buffer[3..SAMPLES], &[0.25; SAMPLES - 3]);
    assert_eq!(buffer[SAMPLES], 4.0);
    assert_eq!(rig.next(slot, 0, 0), 5.0);
}

#[test]
fn untriggered_audio_keeps_its_whole_block() {
    let mut rig = Rig::new(vec![]);
    let slot = rig.alloc_audio();
    rig.fill_samples(slot, 0, 0, 4.0);
    let voice = target(0, 0, None, false);

    rig.with_audio_slot(slot, |ctx, outputs| {
        let mut factory = ctx.audio(MODULE_ID, slot);
        let (router, mut rendered) = factory.for_voice(&voice, outputs);

        assert!(!router.triggered());
        assert_eq!(router.samples(), SAMPLES);
        assert_eq!(rendered.output(), &[4.0; SAMPLES]);
    });

    assert_eq!(rig.arena.samples[slot].at(0, 0).buffer()[0], 4.0);
}

#[test]
fn note_on_at_the_first_sample_does_not_clear_audio() {
    let mut rig = Rig::new(vec![]);
    let slot = rig.alloc_audio();
    rig.fill_samples(slot, 0, 0, 4.0);
    let voice = target(0, 0, Some(0), false);

    rig.with_audio_slot(slot, |ctx, outputs| {
        let mut factory = ctx.audio(MODULE_ID, slot);
        let (router, mut rendered) = factory.for_voice(&voice, outputs);

        assert!(router.triggered());
        assert_eq!(router.samples(), SAMPLES);
        assert_eq!(rendered.output(), &[4.0; SAMPLES]);
    });
}

#[test]
fn audio_direct_starts_at_the_trigger_and_stops_at_the_block_end() {
    let mut rig = Rig::new(vec![]);
    let output = rig.alloc_audio();
    let input = rig.alloc_audio();
    for (index, sample) in rig.sample_buf(input, 1, 2).iter_mut().enumerate() {
        *sample = index as Smp;
    }
    let voice = target(1, 2, Some(2), false);

    let mut ctx = rig.context();
    let mut factory = ctx.audio(MODULE_ID, output);
    let router = factory.router(&voice);
    let direct = router.direct(Some(input));

    assert_eq!(direct.len(), SAMPLES - 2);
    assert_eq!(direct[0], 2.0);
    assert_eq!(direct[direct.len() - 1], (SAMPLES - 1) as Smp);
}

#[test]
fn audio_direct_missing_slot_is_silence() {
    let mut rig = Rig::new(vec![]);
    let output = rig.alloc_audio();
    let voice = target(0, 0, Some(2), false);
    let mut ctx = rig.context();
    let mut factory = ctx.audio(MODULE_ID, output);
    let router = factory.router(&voice);

    assert_eq!(router.direct(None), &[0.0; SAMPLES - 2]);
}

#[test]
fn steady_audio_param_fills_only_the_active_samples() {
    let mut rig = Rig::new(vec![]);
    rig.needs_update_ui = true;
    let slot = rig.alloc_audio();
    let voice = target(0, 0, Some(2), true);
    let mut buff = vec![-1.0; BUFFER_SIZE];
    {
        let mut ctx = rig.context();
        let mut factory = ctx.audio(MODULE_ID, slot);
        factory
            .router(&voice)
            .param(&mixed([]), &SmoothedSample::new(0.5), &mut buff);
    }

    assert_eq!(&buff[..SAMPLES - 2], &[0.5; SAMPLES - 2]);
    assert_eq!(buff[SAMPLES - 2], -1.0);
    assert!(rig.end.updates.is_empty());
}

#[test]
fn smoothing_audio_param_ramps_and_then_adds_modulation() {
    let mut rig = Rig::new(vec![]);
    let slot = rig.alloc_audio();
    let input = rig.alloc_audio();
    let offset = 2;
    for (index, sample) in rig.sample_buf(input, 0, 0).iter_mut().enumerate() {
        *sample = index as Smp;
    }
    let mut param = SmoothedSample::new(0.0);
    param.set(1.0);
    let voice = target(0, 0, Some(offset), false);
    let mut buff = vec![0.0; BUFFER_SIZE];

    {
        let mut ctx = rig.context();
        let mut factory = ctx.audio(MODULE_ID, slot);
        factory.router(&voice).param(
            &mixed([source(input, StereoSample::ONE, None)]),
            &param,
            &mut buff,
        );
    }

    let active = SAMPLES - offset;
    let mut expected = vec![0.0; active];
    param.smoothed_buff(&mut expected, &SmoothedSampleParams::new(SAMPLE_RATE));
    for (index, sample) in expected.iter_mut().enumerate() {
        *sample += (offset + index) as Smp;
    }

    assert_eq!(&buff[..active], expected.as_slice());
    assert_eq!(buff[active], 0.0);
    assert!(rig.end.updates.is_empty());
}

#[test]
fn audio_param_reports_modulation_for_the_last_voice() {
    let mut rig = Rig::new(vec![]);
    rig.needs_update_ui = true;
    let slot = rig.alloc_audio();
    let input = rig.alloc_audio();
    rig.fill_samples(input, 0, 0, 0.5);
    let amount = StereoSample::new(4.0, 1.0);
    let sources = mixed([source(input, amount, None)]);
    let param = SmoothedSample::new(10.0);

    let ignored_voice = target(0, 0, None, false);
    let reported_voice = target(0, 0, None, true);
    let mut ignored = [0.0; SAMPLES];
    let mut buff = [0.0; SAMPLES];
    {
        let mut ctx = rig.context();
        let mut factory = ctx.audio(MODULE_ID, slot);
        factory
            .router(&ignored_voice)
            .param(&sources, &param, &mut ignored);
        factory
            .router(&reported_voice)
            .param(&sources, &param, &mut buff);
    }

    assert_eq!(buff, [12.0; SAMPLES]);
    assert_eq!(rig.end.updates.len(), 1);
    assert_update(
        &rig.end.updates[0],
        0,
        12.0,
        sources.normalized_modulated(0, 2.0),
    );
}

#[test]
fn audio_scalar_reads_this_frame_or_the_held_sample() {
    let mut rig = Rig::new(vec![]);
    let slot = rig.alloc_audio();
    let input = rig.alloc_control();
    rig.fill_samples(input, 0, 0, 100.0);
    rig.sample_buf(input, 0, 0)[2] = 3.0;
    rig.set_next(input, 0, 0, 8.0);
    let sources = mixed([source(input, StereoSample::ONE, None)]);
    let voice = target(0, 0, Some(2), false);

    let mut ctx = rig.context();
    let mut factory = ctx.audio(MODULE_ID, slot);
    let mut router = factory.router(&voice);

    assert_eq!(router.scalar(&sources, 1.0, true), 4.0);
    assert_eq!(router.scalar(&sources, 1.0, false), 9.0);
    assert_eq!(router.scalar(&mixed([]), 6.0, true), 6.0);
}

#[test]
fn audio_scalar_sums_sources_and_modulators() {
    let mut rig = Rig::new(vec![]);
    let slot = rig.alloc_audio();
    let left = rig.alloc_control();
    let right = rig.alloc_control();
    let modulator = rig.alloc_control();
    rig.fill_samples(left, 0, 0, 3.0);
    rig.fill_samples(left, 1, 0, 3.0);
    rig.fill_samples(right, 0, 0, 5.0);
    rig.fill_samples(right, 1, 0, 5.0);
    rig.fill_samples(modulator, 0, 0, 4.0);
    rig.fill_samples(modulator, 1, 0, 4.0);
    let sources = mixed([
        source(left, StereoSample::new(2.0, 5.0), Some(modulator)),
        source(right, StereoSample::ONE, None),
    ]);

    let left_voice = target(0, 0, None, false);
    let right_voice = target(1, 0, None, false);
    let mut ctx = rig.context();
    let mut factory = ctx.audio(MODULE_ID, slot);

    assert_eq!(
        factory.router(&left_voice).scalar(&sources, 1.0, true),
        30.0
    );
    assert_eq!(
        factory.router(&right_voice).scalar(&sources, 1.0, true),
        66.0
    );
}

#[test]
fn audio_scalar_reports_modulation_only_for_the_selected_voice() {
    let mut rig = Rig::new(vec![]);
    rig.needs_update_ui = true;
    let slot = rig.alloc_audio();
    let input = rig.alloc_control();
    rig.fill_samples(input, 0, 0, 0.5);
    rig.fill_samples(input, 1, 0, 0.5);
    let sources = mixed([source(input, StereoSample::new(4.0, 1.0), None)]);
    let selected = target(0, 0, None, true);
    let other = target(0, 0, None, false);
    let right = target(1, 0, None, true);

    {
        let mut ctx = rig.context();
        let mut factory = ctx.audio(MODULE_ID, slot);
        assert_eq!(factory.router(&selected).scalar(&mixed([]), 4.0, true), 4.0);
        assert_eq!(factory.router(&other).scalar(&sources, 10.0, true), 12.0);
    }
    assert!(rig.end.updates.is_empty());

    rig.needs_update_ui = false;
    {
        let mut ctx = rig.context();
        let mut factory = ctx.audio(MODULE_ID, slot);
        assert_eq!(factory.router(&selected).scalar(&sources, 10.0, true), 12.0);
    }
    assert!(rig.end.updates.is_empty());

    rig.needs_update_ui = true;
    {
        let mut ctx = rig.context();
        let mut factory = ctx.audio(MODULE_ID, slot);
        assert_eq!(factory.router(&right).scalar(&sources, 10.0, true), 10.5);
    }
    assert_eq!(rig.end.updates.len(), 1);
    assert_update(
        &rig.end.updates[0],
        1,
        10.5,
        sources.normalized_modulated(1, 0.5),
    );

    rig.end.updates.clear();
    {
        let mut ctx = rig.context();
        let mut factory = ctx.audio(MODULE_ID, slot);
        assert_eq!(factory.router(&selected).scalar(&sources, 10.0, true), 12.0);
    }
    assert_update(
        &rig.end.updates[0],
        0,
        12.0,
        sources.normalized_modulated(0, 2.0),
    );
}

#[test]
fn param_stationary_requires_a_settled_unmodulated_value() {
    let mut rig = Rig::new(vec![]);
    let slot = rig.alloc_audio();
    let input = rig.alloc_audio();
    let settled = SmoothedSample::new(0.3);
    let mut moving = SmoothedSample::new(0.3);
    moving.set(0.8);
    let voice = target(0, 0, None, false);
    let mut ctx = rig.context();
    let mut factory = ctx.audio(MODULE_ID, slot);
    let router = factory.router(&voice);

    assert_eq!(router.param_stationary(&mixed([]), &settled), Some(0.3));
    assert_eq!(router.param_stationary(&mixed([]), &moving), None);
    assert_eq!(
        router.param_stationary(&mixed([source(input, StereoSample::ONE, None)]), &settled),
        None
    );
}

#[test]
fn control_for_voices_skips_triggered_voices_until_the_trigger_stage() {
    let mut rig = Rig::new(mixed_voices());
    let slot = rig.alloc_control();
    rig.fill_samples(slot, 0, 0, 1.5);

    let mut process = Vec::new();
    let mut ctx = rig.context();
    ctx.control(MODULE_ID, slot)
        .for_voices(|factory, voice, _| {
            assert!(factory.ctx.outputs_arena.samples[slot].slot.is_none());
            process.push(visit(voice));
        });

    assert_eq!(
        process,
        [
            Visit {
                voice: 1,
                channel: 0,
                triggered: None,
            },
            Visit {
                voice: 1,
                channel: 1,
                triggered: None,
            },
        ]
    );

    rig.trigger_stage = true;
    let mut trigger = Vec::new();
    let mut ctx = rig.context();
    ctx.control(MODULE_ID, slot).for_voices(|_, voice, _| {
        trigger.push(visit(voice));
    });

    assert_eq!(
        trigger,
        [
            Visit {
                voice: 0,
                channel: 0,
                triggered: Some(4),
            },
            Visit {
                voice: 1,
                channel: 0,
                triggered: None,
            },
            Visit {
                voice: 0,
                channel: 1,
                triggered: Some(4),
            },
            Visit {
                voice: 1,
                channel: 1,
                triggered: None,
            },
        ]
    );
    assert_eq!(rig.arena.samples[slot].at(0, 0).buffer()[0], 1.5);
}

#[test]
#[should_panic(expected = "slot should be in place")]
fn control_for_voices_panics_when_the_slot_is_freed() {
    let mut rig = Rig::new(vec![PlayingVoice::new(0, 60)]);
    let slot = rig.alloc_control();
    rig.free(slot, DataType::Control);

    let mut ctx = rig.context();
    ctx.control(MODULE_ID, slot).for_voices(|_, _, _| {});
}

#[test]
fn control_for_voice_seeds_an_untriggered_block_and_clears_a_trigger_prefix() {
    let mut rig = Rig::new(vec![]);
    let slot = rig.alloc_control();

    rig.fill_samples(slot, 0, 0, 4.0);
    rig.set_next(slot, 0, 0, 9.0);
    let steady = target(0, 0, None, false);
    rig.with_control_slot(slot, |ctx, outputs| {
        let mut factory = ctx.control(MODULE_ID, slot);
        let (router, mut rendered) = factory.for_voice(&steady, outputs);

        assert!(!router.triggered());
        assert_eq!(router.block_to_voice_offset(0), 0);
        assert_eq!(router.block_to_voice_offset(1), 0);
        assert_eq!(router.block_to_voice_offset(4), 3);
        assert_eq!(rendered.output().len(), SAMPLES);
        assert_eq!(rendered.output()[0], 4.0);
    });
    assert_eq!(rig.arena.samples[slot].at(0, 0).buffer()[0], 9.0);
    assert_eq!(rig.arena.samples[slot].at(0, 0).buffer()[1], 4.0);

    rig.fill_samples(slot, 0, 0, 4.0);
    rig.set_next(slot, 0, 0, 9.0);
    let at_start = target(0, 0, Some(0), false);
    rig.with_control_slot(slot, |ctx, outputs| {
        let mut factory = ctx.control(MODULE_ID, slot);
        let (router, mut rendered) = factory.for_voice(&at_start, outputs);

        assert!(router.triggered());
        assert_eq!(router.block_to_voice_offset(0), 0);
        assert_eq!(rendered.output().len(), SAMPLES + CONTROL_EXT);
        assert_eq!(rendered.output()[0], 4.0);
    });
    assert_eq!(rig.arena.samples[slot].at(0, 0).buffer()[0], 4.0);

    rig.fill_samples(slot, 0, 0, 4.0);
    let later = target(0, 0, Some(3), false);
    rig.with_control_slot(slot, |ctx, outputs| {
        let mut factory = ctx.control(MODULE_ID, slot);
        let (router, mut rendered) = factory.for_voice(&later, outputs);

        assert_eq!(router.block_to_voice_offset(5), 2);
        assert_eq!(router.block_to_voice_offset(1), 0);
        assert_eq!(rendered.output().len(), SAMPLES - 3 + CONTROL_EXT);
        rendered.output().fill(0.25);
    });

    let buffer = rig.arena.samples[slot].at(0, 0).buffer();
    assert_eq!(&buffer[..3], &[0.0; 3]);
    assert_eq!(
        &buffer[3..SAMPLES + CONTROL_EXT],
        &[0.25; SAMPLES - 3 + CONTROL_EXT]
    );
    assert_eq!(rig.next(slot, 0, 0), 0.25);
}

#[test]
fn control_param_applies_from_the_trigger_offset() {
    let mut rig = Rig::new(vec![]);
    let slot = rig.alloc_control();
    let input = rig.alloc_control();
    for (index, sample) in rig.sample_buf(input, 0, 0).iter_mut().enumerate() {
        *sample = index as Smp;
    }
    let sources = mixed([source(input, StereoSample::ONE, None)]);
    let param = SmoothedSample::new(10.0);

    let mut untriggered = zero_buffer(BUFFER_SIZE);
    untriggered.fill(-1.0);
    let steady = target(0, 0, None, false);
    rig.with_control_slot(slot, |ctx, outputs| {
        let mut factory = ctx.control(MODULE_ID, slot);
        let (mut router, _) = factory.for_voice(&steady, outputs);
        router.param(&sources, &param, &mut untriggered);
    });
    assert_eq!(
        &untriggered[..SAMPLES],
        &(1..SAMPLES + 1)
            .map(|index| 10.0 + index as Smp)
            .collect::<Vec<_>>()
    );
    assert_eq!(untriggered[SAMPLES], -1.0);

    let mut moving = SmoothedSample::new(0.0);
    moving.set(1.0);
    let mut smoothed = zero_buffer(BUFFER_SIZE);
    let at_start = target(0, 0, Some(0), false);
    rig.with_control_slot(slot, |ctx, outputs| {
        let mut factory = ctx.control(MODULE_ID, slot);
        let (mut router, _) = factory.for_voice(&at_start, outputs);
        router.param(&mixed([]), &moving, &mut smoothed);
    });
    let active = SAMPLES + CONTROL_EXT;
    let mut expected = vec![0.0; active];
    moving.smoothed_buff(&mut expected, &SmoothedSampleParams::new(SAMPLE_RATE));
    assert_eq!(&smoothed[..active], expected.as_slice());
    assert_eq!(smoothed[active], 0.0);
}

#[test]
fn control_param_reports_modulation_for_the_last_voice() {
    let mut rig = Rig::new(vec![]);
    rig.needs_update_ui = true;
    let slot = rig.alloc_control();
    let input = rig.alloc_control();
    rig.fill_samples(input, 0, 0, 0.5);
    let sources = mixed([source(input, StereoSample::new(4.0, 0.0), None)]);
    let param = SmoothedSample::new(10.0);
    let voice = target(0, 0, Some(0), true);

    let mut buff = zero_buffer(BUFFER_SIZE);
    rig.with_control_slot(slot, |ctx, outputs| {
        let mut factory = ctx.control(MODULE_ID, slot);
        let (mut router, _) = factory.for_voice(&voice, outputs);
        router.param(&sources, &param, &mut buff);
    });

    assert_eq!(
        &buff[..SAMPLES + CONTROL_EXT],
        &[12.0; SAMPLES + CONTROL_EXT]
    );
    assert_eq!(rig.end.updates.len(), 1);
    assert_update(
        &rig.end.updates[0],
        0,
        12.0,
        sources.normalized_modulated(0, 2.0),
    );
}

#[test]
fn control_scalar_reads_the_sample_at_its_offset() {
    let mut rig = Rig::new(vec![]);
    let slot = rig.alloc_control();
    let input = rig.alloc_control();
    let buffer = rig.sample_buf(input, 0, 0);
    buffer.fill(0.0);
    buffer[0] = 2.0;
    buffer[1] = 3.0;
    buffer[4] = 6.0;
    rig.set_next(input, 0, 0, 8.0);
    let sources = mixed([source(input, StereoSample::ONE, None)]);

    let read = |rig: &mut Rig, triggered: Option<usize>| -> Smp {
        let voice = target(0, 0, triggered, false);
        rig.with_control_slot(slot, |ctx, outputs| {
            let mut factory = ctx.control(MODULE_ID, slot);
            let (mut router, _) = factory.for_voice(&voice, outputs);
            router.scalar(&sources, 0.0)
        })
    };

    assert_eq!(read(&mut rig, None), 3.0);
    assert_eq!(read(&mut rig, Some(0)), 2.0);
    assert_eq!(read(&mut rig, Some(4)), 6.0);
}

#[test]
fn external_control_is_block_aligned_and_holds_its_last_sample() {
    let mut rig = Rig::new(vec![]);
    let slot = rig.alloc_control();
    rig.fill_samples(slot, 0, 0, 0.5);
    rig.set_next(slot, 0, 0, 0.5);
    let input = [1.0, 2.0, 3.0, 4.0];

    let steady = target(0, 0, None, false);
    rig.with_control_slot(slot, |ctx, outputs| {
        let mut factory = ctx.control(MODULE_ID, slot);
        let (_, mut rendered) = factory.for_voice(&steady, outputs);
        rendered.fill_with_ext_control(&input);
    });

    let buffer = rig.arena.samples[slot].at(0, 0).buffer();
    assert_eq!(&buffer[..4], &input);
    assert_eq!(buffer[4], 4.0);
    assert_eq!(buffer[5], 0.5);
    assert_eq!(rig.next(slot, 0, 0), 0.5);
}

#[test]
fn triggered_external_control_starts_at_the_trigger() {
    let mut rig = Rig::new(vec![]);
    let slot = rig.alloc_control();
    rig.fill_samples(slot, 0, 0, 0.5);
    let input = [9.0, 8.0, 7.0, 6.0, 5.0, 4.0, 3.0, 2.0];

    let triggered = target(0, 0, Some(2), false);
    rig.with_control_slot(slot, |ctx, outputs| {
        let mut factory = ctx.control(MODULE_ID, slot);
        let (_, mut rendered) = factory.for_voice(&triggered, outputs);
        rendered.fill_with_ext_control(&input);
    });

    let buffer = rig.arena.samples[slot].at(0, 0).buffer();
    assert_eq!(&buffer[..2], &[0.0; 2]);
    assert_eq!(&buffer[2..8], &[7.0, 6.0, 5.0, 4.0, 3.0, 2.0]);
    assert_eq!(buffer[8], 2.0);
    assert_eq!(rig.next(slot, 0, 0), 2.0);
}

#[test]
fn spectral_for_voices_visits_every_voice_on_both_stages() {
    for trigger_stage in [false, true] {
        let mut rig = Rig::new(mixed_voices());
        rig.trigger_stage = trigger_stage;
        let slot = rig.alloc_spectral();
        let bins = [ComplexSample::new(1.0, -1.0)];
        rig.set_spectrum(slot, 0, 0, &bins);

        let mut visits = Vec::new();
        let mut ctx = rig.context();
        ctx.spectral(MODULE_ID, slot)
            .for_voices(|factory, voice, outputs| {
                assert!(factory.ctx.outputs_arena.spectral[slot].slot.is_none());
                assert_eq!(outputs.at(0, 0).get(), &bins);
                visits.push(visit(voice));
            });

        assert_eq!(
            visits,
            [
                Visit {
                    voice: 0,
                    channel: 0,
                    triggered: Some(4),
                },
                Visit {
                    voice: 1,
                    channel: 0,
                    triggered: None,
                },
                Visit {
                    voice: 0,
                    channel: 1,
                    triggered: Some(4),
                },
                Visit {
                    voice: 1,
                    channel: 1,
                    triggered: None,
                },
            ]
        );
    }
}

#[test]
#[should_panic(expected = "slot should be in place")]
fn spectral_for_voices_panics_when_the_slot_is_freed() {
    let mut rig = Rig::new(vec![PlayingVoice::new(0, 60)]);
    let slot = rig.alloc_spectral();
    rig.free(slot, DataType::Spectral);

    let mut ctx = rig.context();
    ctx.spectral(MODULE_ID, slot).for_voices(|_, _, _| {});
}

#[test]
fn spectral_output_sets_and_clamps_the_spectrum_length() {
    let mut rig = Rig::new(vec![]);
    let slot = rig.alloc_spectral();
    let voice = target(0, 0, None, false);

    rig.with_spectral_slot(slot, |ctx, outputs| {
        let mut factory = ctx.spectral(MODULE_ID, slot);
        let (_, mut rendered) = factory.for_voice(&voice, outputs);
        let spectrum = rendered.output(2);
        spectrum[0] = ComplexSample::new(1.0, 2.0);
        spectrum[1] = ComplexSample::new(3.0, 4.0);
    });

    assert_eq!(
        rig.arena.spectral[slot].at(0, 0).get(),
        &[ComplexSample::new(1.0, 2.0), ComplexSample::new(3.0, 4.0),]
    );

    rig.with_spectral_slot(slot, |ctx, outputs| {
        let mut factory = ctx.spectral(MODULE_ID, slot);
        let (_, mut rendered) = factory.for_voice(&voice, outputs);
        assert_eq!(
            rendered.output(SPECTRAL_BUFFER_SIZE + 10).len(),
            SPECTRAL_BUFFER_SIZE
        );
    });
    assert_eq!(
        rig.arena.spectral[slot].at(0, 0).get().len(),
        SPECTRAL_BUFFER_SIZE
    );
}

#[test]
fn router_spectral_reads_the_voice_spectrum() {
    let mut rig = Rig::new(vec![]);
    let audio = rig.alloc_audio();
    let spectral = rig.alloc_spectral();
    let bins = [ComplexSample::new(1.0, 2.0), ComplexSample::new(3.0, 4.0)];
    rig.set_spectrum(spectral, 1, 2, &bins);

    let voice = target(1, 2, None, false);
    let mut ctx = rig.context();
    let mut factory = ctx.audio(MODULE_ID, audio);
    let router = factory.router(&voice);

    assert_eq!(router.spectral(Some(spectral)), &bins);
    assert!(router.spectral(None).is_empty());
}

#[test]
fn spectral_scalar_follows_the_trigger_stage() {
    let mut rig = Rig::new(vec![]);
    let spectral = rig.alloc_spectral();
    let input = rig.alloc_control();
    let buffer = rig.sample_buf(input, 0, 0);
    buffer.fill(0.0);
    buffer[2] = 3.0;
    rig.set_next(input, 0, 0, 8.0);
    let sources = mixed([source(input, StereoSample::ONE, None)]);

    let read = |rig: &mut Rig, stage: bool, triggered: Option<usize>| -> (Option<Smp>, Smp) {
        rig.trigger_stage = stage;
        let voice = target(0, 0, triggered, true);
        rig.with_spectral_slot(spectral, |ctx, outputs| {
            let mut factory = ctx.spectral(MODULE_ID, spectral);
            let (mut router, _) = factory.for_voice(&voice, outputs);
            (
                router.direct_opt(Some(input)),
                router.scalar(&sources, 10.0),
            )
        })
    };

    assert_eq!(read(&mut rig, false, Some(2)), (Some(8.0), 18.0));
    assert_eq!(read(&mut rig, true, Some(2)), (Some(3.0), 13.0));
    assert_eq!(read(&mut rig, true, None), (Some(8.0), 18.0));

    rig.trigger_stage = true;
    rig.needs_update_ui = true;
    let voice = target(0, 0, None, true);
    rig.with_spectral_slot(spectral, |ctx, outputs| {
        let mut factory = ctx.spectral(MODULE_ID, spectral);
        let (mut router, _) = factory.for_voice(&voice, outputs);
        assert_eq!(router.direct_opt(None), None);
        assert_eq!(router.scalar(&mixed([]), 4.0), 4.0);
    });
    assert!(rig.end.updates.is_empty());
}

#[test]
fn output_direct_is_the_whole_block_even_when_the_voice_is_triggered() {
    let mut rig = Rig::new(vec![]);
    let input = rig.alloc_audio();
    for (index, sample) in rig.sample_buf(input, 0, 1).iter_mut().enumerate() {
        *sample = index as Smp;
    }

    let voice = target(0, 1, Some(3), false);
    let mut ctx = rig.context();
    let mut factory = ctx.for_output(MODULE_ID);
    let mut router = factory.for_voice(&voice);
    let direct = router.direct(Some(input));

    assert_eq!(direct.len(), SAMPLES);
    assert_eq!(direct[0], 0.0);
    assert_eq!(direct[SAMPLES - 1], (SAMPLES - 1) as Smp);
}

#[test]
fn output_direct_missing_slot_is_silence() {
    let mut rig = Rig::new(vec![]);
    let voice = target(0, 0, None, false);
    let mut ctx = rig.context();
    let mut factory = ctx.for_output(MODULE_ID);
    let mut router = factory.for_voice(&voice);

    assert_eq!(router.direct(None), &[0.0; SAMPLES]);
}

#[test]
fn dropping_an_output_voice_leaves_samples_in_place() {
    let mut rig = Rig::new(vec![]);
    let slot = rig.alloc_audio();
    rig.fill_samples(slot, 0, 0, 3.0);
    rig.set_next(slot, 0, 0, 6.0);

    rig.with_audio_slot(slot, |_ctx, outputs| {
        let voice = target(0, 0, Some(2), true);
        let _rendered = VoiceOutput::<OutputRouterType> {
            outputs,
            target: &voice,
            state: OutputVoiceState,
            samples: SAMPLES,
        };
    });

    assert_eq!(rig.arena.samples[slot].at(0, 0).buffer()[0], 3.0);
    assert_eq!(rig.next(slot, 0, 0), 6.0);
}
