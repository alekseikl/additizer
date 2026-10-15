use std::array;

mod config;
mod link;
pub mod stub;

#[cfg(test)]
mod tests;

pub use config::SvfConfig;
pub use link::{SvfAudioEnd, SvfLinks, SvfUiEnd, UiEvent};

use addi_dsp::filters::{
    control::{MAX_DRIVE, MAX_RESONANCE, MIN_DRIVE, MIN_RESONANCE, q_from_resonance},
    svf::{LaneInputs, SvfState, SvfType},
};

use crate::{
    synth_engine::{
        C4_PITCH, MAX_CUTOFF, MIN_CUTOFF, SIMD_LANES, db_to_gain_fast, db_to_gain_fast_wide,
        map_wide_in_place,
    },
    synth_engine::{
        SmoothedSampleParams, Smp, StereoSample, WideSmp,
        buffer::{Buffer, VoicesLayout, zero_buffer},
        routing::{
            AudioRouterType, DataType, Input, InputMeta, InputSlots, MixedSlots, ModuleId,
            ProcessContext, RouterFactory, SamplesOutput, VoiceEvent, VoiceTarget, VoicesBlock,
        },
        smooth::SmoothedSample,
        synth_module::SynthModule,
        voices::{NUM_CHANNELS, VOICE_BLOCK_LEN, VOICE_BLOCKS},
    },
};

struct Params {
    filter_type: SvfType,
    keytrack: Smp,
}

impl Params {
    fn from_config(c: &SvfConfig) -> Self {
        Self {
            filter_type: c.filter_type,
            keytrack: c.keytrack.clamp(0.0, 1.0),
        }
    }
}

struct ChannelParams {
    cutoff: SmoothedSample,
    resonance: SmoothedSample,
    drive: SmoothedSample,
}

impl ChannelParams {
    fn from_config(c: &SvfConfig, channel_idx: usize) -> Self {
        Self {
            cutoff: c.cutoff[channel_idx].into(),
            resonance: c.resonance[channel_idx]
                .clamp(MIN_RESONANCE, MAX_RESONANCE)
                .into(),
            drive: c.drive[channel_idx].into(),
        }
    }

    pub fn advance_smoothers(&mut self, smooth_params: &SmoothedSampleParams, samples: usize) {
        self.cutoff.advance(smooth_params, samples);
        self.resonance.advance(smooth_params, samples);
        self.drive.advance(smooth_params, samples);
    }
}

pub struct Inputs {
    audio: Option<usize>,
    pitch: Option<usize>,
    cutoff: MixedSlots,
    resonance: MixedSlots,
    drive: MixedSlots,
}

impl Default for Inputs {
    fn default() -> Self {
        Self {
            audio: None,
            pitch: None,
            cutoff: MixedSlots::new(Input::Cutoff),
            resonance: MixedSlots::new(Input::Resonance),
            drive: MixedSlots::new(Input::Drive),
        }
    }
}

impl Inputs {
    fn from_slots(inputs: &[InputSlots]) -> Self {
        let mut result = Self::default();

        for input in inputs {
            match input {
                InputSlots::Direct { input_type, slot } => match input_type {
                    Input::Audio => result.audio = Some(*slot),
                    Input::Pitch => result.pitch = Some(*slot),
                    _ => (),
                },
                InputSlots::Mixed(input) => match input.input_type {
                    Input::Cutoff => result.cutoff = input.clone(),
                    Input::Resonance => result.resonance = input.clone(),
                    Input::Drive => result.drive = input.clone(),
                    _ => (),
                },
                InputSlots::Spectral { .. } => (),
            }
        }

        result
    }

    fn update_amount(&mut self, input_type: Input, src_slot: usize, amount: StereoSample) {
        match input_type {
            Input::Cutoff => self.cutoff.update_amount(src_slot, amount),
            Input::Resonance => self.resonance.update_amount(src_slot, amount),
            Input::Drive => self.drive.update_amount(src_slot, amount),
            _ => (),
        }
    }
}

struct Buffers {
    audio: [Buffer; SIMD_LANES],
    cutoff: [Buffer; SIMD_LANES],
    resonance: [Buffer; SIMD_LANES],
    pre_k: [Buffer; SIMD_LANES],
    drive: [Buffer; SIMD_LANES],
}

impl Buffers {
    fn new(buffer_size: usize) -> Self {
        let lanes = || array::from_fn(|_| zero_buffer(buffer_size));
        Self {
            audio: lanes(),
            cutoff: lanes(),
            resonance: lanes(),
            pre_k: lanes(),
            drive: lanes(),
        }
    }
}

pub struct Svf<L: SvfLinks = stub::Links> {
    id: ModuleId,
    params: Params,
    channel_params: [ChannelParams; NUM_CHANNELS],
    buffers: Buffers,
    states: [SvfState; VOICE_BLOCKS],
    audio_end: L::AudioEnd,
    ui_end: Option<L::UiEnd>,
    inputs: Inputs,
    output_slot: usize,
}

impl<L: SvfLinks> Svf<L> {
    pub fn take_ui_end(&mut self) -> Option<L::UiEnd> {
        self.ui_end.take()
    }

    pub fn new(id: ModuleId, buffer_size: usize) -> Self {
        Self::from_config(
            &SvfConfig {
                id,
                ..SvfConfig::default()
            },
            buffer_size,
        )
    }

    pub fn from_config(config: &SvfConfig, buffer_size: usize) -> Self {
        let (audio_end, ui_end) = L::create_link_pair();

        Self {
            id: config.id,
            params: Params::from_config(config),
            channel_params: array::from_fn(|channel_idx| {
                ChannelParams::from_config(config, channel_idx)
            }),
            buffers: Buffers::new(buffer_size),
            states: array::from_fn(|_| SvfState::new(config.filter_type)),
            audio_end,
            ui_end,
            inputs: Inputs::default(),
            output_slot: usize::MAX,
        }
    }

    pub fn get_config(&self) -> SvfConfig {
        SvfConfig {
            id: self.id,
            filter_type: self.params.filter_type,
            keytrack: self.params.keytrack,
            cutoff: get_smoothed_param!(self, cutoff),
            resonance: get_smoothed_param!(self, resonance),
            drive: get_smoothed_param!(self, drive),
        }
    }

    set_mono_param!(set_filter_type, filter_type, SvfType);
    set_mono_param!(set_keytrack, keytrack, Smp, keytrack.clamp(0.0, 1.0));

    set_smoothed_param!(set_cutoff, cutoff, cutoff.clamp(MIN_CUTOFF, MAX_CUTOFF));
    set_smoothed_param!(
        set_resonance,
        resonance,
        resonance.clamp(MIN_RESONANCE, MAX_RESONANCE)
    );
    set_smoothed_param!(set_drive, drive, drive.clamp(MIN_DRIVE, MAX_DRIVE));

    fn reset_voice(&mut self, voice_idx: usize) {
        let pair_idx = voice_idx / VOICE_BLOCK_LEN;
        let slot = voice_idx % VOICE_BLOCK_LEN;
        let filter_type = self.params.filter_type;
        let state = &mut self.states[pair_idx];

        state.set_type(filter_type);
        state.reset(slot);
    }

    fn prepare_voice(
        &mut self,
        rf: &mut RouterFactory<AudioRouterType, L::EngineEnd>,
        target: &VoiceTarget,
        lane_idx: usize,
    ) {
        let mut router = rf.router(target);
        let keytrack = self.params.keytrack;
        let lane = router.lane(lane_idx);
        let channel = &self.channel_params[target.channel_idx];

        let audio = lane.buff(&mut self.buffers.audio);
        let cutoff = lane.buff(&mut self.buffers.cutoff);
        let resonance = lane.buff(&mut self.buffers.resonance);
        let pre_k = lane.buff(&mut self.buffers.pre_k);
        let drive = lane.buff(&mut self.buffers.drive);

        router.param(&self.inputs.cutoff, &channel.cutoff, cutoff);

        if let Some(resonance_value) =
            router.param_stationary(&self.inputs.resonance, &channel.resonance)
        {
            let q = q_from_resonance(resonance_value);

            resonance.fill(q.resonant.recip());
            pre_k.fill(q.pre_stage.recip());
        } else {
            router.param(&self.inputs.resonance, &channel.resonance, resonance);

            for (resonance, pre_k) in resonance.iter_mut().zip(pre_k.iter_mut()) {
                let q = q_from_resonance(*resonance);

                *resonance = q.resonant.recip();
                *pre_k = q.pre_stage.recip();
            }
        }

        if let Some(drive_value) = router.param_stationary(&self.inputs.drive, &channel.drive) {
            let gain = db_to_gain_fast(drive_value.clamp(MIN_DRIVE, MAX_DRIVE));

            drive.fill(gain);
        } else {
            router.param(&self.inputs.drive, &channel.drive, drive);

            const MIN_DRIVE_X4: WideSmp = WideSmp::splat(MIN_DRIVE);
            const MAX_DRIVE_X4: WideSmp = WideSmp::splat(MAX_DRIVE);

            map_wide_in_place(drive, |drive| {
                db_to_gain_fast_wide(drive.fast_max(MIN_DRIVE_X4).fast_min(MAX_DRIVE_X4))
            });
        }

        let pitch = router.direct(self.inputs.pitch);
        let audio_in = router.direct(self.inputs.audio);

        if router.need_update_ui_mono() {
            self.audio_end.update_pitch(pitch[0]);
        }

        audio.copy_from_slice(audio_in);

        if keytrack > 1e-5 {
            for (cutoff, pitch) in cutoff.iter_mut().zip(pitch) {
                *cutoff += keytrack * (pitch - C4_PITCH);
            }
        }
    }

    fn process_block(
        &mut self,
        rf: &mut RouterFactory<AudioRouterType, L::EngineEnd>,
        block: &VoicesBlock,
        outputs: &mut VoicesLayout<SamplesOutput>,
    ) {
        let block_idx = block.block_idx;
        let filter_type = self.params.filter_type;
        let buffers = &self.buffers;
        let state = &mut self.states[block_idx];
        let params = rf.params();

        state.set_type(filter_type);
        state.process(
            params.sample_rate,
            params.samples,
            LaneInputs {
                audio: block_refs(&buffers.audio),
                cutoff: block_refs(&buffers.cutoff),
                k: block_refs(&buffers.resonance),
                pre_k: block_refs(&buffers.pre_k),
                gain: block_refs(&buffers.drive),
            },
            outputs.buffers_block_mut(block_idx),
        );
    }

    pub fn process(&mut self, ctx: &mut ProcessContext<L::EngineEnd>) {
        ctx.audio(self.id, self.output_slot)
            .for_lanes(self, Self::prepare_voice, Self::process_block)
            .for_channels(|rf, channel_idx| {
                self.channel_params[channel_idx]
                    .advance_smoothers(&rf.params().smooth_params, rf.params().samples);
            });
    }
}

fn block_refs(buffers: &[Buffer; SIMD_LANES]) -> [&[Smp]; SIMD_LANES] {
    buffers.each_ref().map(|buffer| &buffer[..])
}

impl<L: SvfLinks> SynthModule for Svf<L> {
    fn id(&self) -> ModuleId {
        self.id
    }

    fn inputs(&self) -> &'static [InputMeta] {
        static INPUTS: &[InputMeta] = &[
            InputMeta::direct_audio(Input::Audio),
            InputMeta::direct_control(Input::Pitch),
            InputMeta::control(Input::Cutoff),
            InputMeta::control(Input::Resonance),
            InputMeta::control(Input::Drive),
        ];

        INPUTS
    }

    fn output_type(&self) -> DataType {
        DataType::Audio
    }

    fn output_slot(&self) -> usize {
        self.output_slot
    }

    fn set_output_slot(&mut self, slot: usize) {
        self.output_slot = slot;
    }

    fn set_input_slots(&mut self, inputs: &[InputSlots]) {
        self.inputs = Inputs::from_slots(inputs);
    }

    fn update_input_amount(&mut self, input_type: Input, src_slot: usize, amount: StereoSample) {
        self.inputs.update_amount(input_type, src_slot, amount);
    }

    fn process_events(&mut self, events: &[VoiceEvent]) {
        for event in events {
            if let VoiceEvent::Reset { voice_idx, .. } = event {
                self.reset_voice(*voice_idx);
            }
        }
    }

    fn process_ui_events(&mut self) {
        while let Some(event) = self.audio_end.pop_event() {
            match event {
                UiEvent::InputParam { input, value } => match input {
                    Input::Cutoff => self.set_cutoff(value),
                    Input::Resonance => self.set_resonance(value),
                    Input::Drive => self.set_drive(value),
                    _ => (),
                },
                UiEvent::FilterType(filter_type) => self.set_filter_type(filter_type),
                UiEvent::Keytrack(value) => self.set_keytrack(value),
            }
        }
    }
}
