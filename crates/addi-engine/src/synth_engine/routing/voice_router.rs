use crate::synth_engine::{
    Buffer, ComplexSample, ModuleId, NUM_CHANNELS, ProcessParams, Sample,
    buffer::{VoicesLayout, ZEROES_BUFFER},
    engine_io::EngineAudioEnd,
    routing::{
        MixedSlots, ProcessContext, SamplesOutput, SpectralOutput, VOICE_PAIR_LEN, VOICE_PAIRS,
        process_context::VoiceTarget,
    },
    smooth::SmoothedSample,
    voices_handler::PlayingVoice,
};

pub trait RouterDataType {
    type OutputType;
    type VoiceState: Copy;

    fn advance(
        outputs: &mut VoicesLayout<Self::OutputType>,
        target: &VoiceTarget,
        state: Self::VoiceState,
        samples: usize,
    );
}

#[derive(Clone, Copy)]
pub struct AudioVoiceState {
    offset: usize,
}

pub struct AudioRouterType {
    pub(super) samples_slot: usize,
}

/// One SIMD pair. `voices[0]` is the even slot and `voices[1]` the odd slot.
#[derive(Clone, Copy)]
pub struct VoicesBlock {
    pub pair_idx: usize,
    pub voices: [Option<PlayingVoice>; VOICE_PAIR_LEN],
    pub last_voice_idx: Option<usize>,
}

impl RouterDataType for AudioRouterType {
    type OutputType = SamplesOutput;
    type VoiceState = AudioVoiceState;

    fn advance(
        _outputs: &mut VoicesLayout<Self::OutputType>,
        _target: &VoiceTarget,
        _state: Self::VoiceState,
        _samples: usize,
    ) {
    }
}

#[derive(Clone, Copy)]
pub struct ControlVoiceState {
    offset: usize,
}

pub struct ControlRouterType {
    pub(super) samples_slot: usize,
}

impl RouterDataType for ControlRouterType {
    type OutputType = SamplesOutput;
    type VoiceState = ControlVoiceState;

    fn advance(
        outputs: &mut VoicesLayout<Self::OutputType>,
        target: &VoiceTarget,
        _state: Self::VoiceState,
        samples: usize,
    ) {
        let output = outputs.at_mut(target.channel_idx, target.voice_idx);

        output.next_frame_sample = output.buffer[samples];
    }
}

#[derive(Clone, Copy)]
pub struct SpectralVoiceState;

pub struct SpectralRouterType {
    pub(super) spectral_slot: usize,
}

impl RouterDataType for SpectralRouterType {
    type OutputType = SpectralOutput;
    type VoiceState = SpectralVoiceState;

    fn advance(
        _outputs: &mut VoicesLayout<Self::OutputType>,
        _target: &VoiceTarget,
        _state: Self::VoiceState,
        _samples: usize,
    ) {
    }
}

#[derive(Clone, Copy)]
pub struct OutputVoiceState;

pub struct OutputRouterType;

impl RouterDataType for OutputRouterType {
    type OutputType = SamplesOutput;
    type VoiceState = OutputVoiceState;

    fn advance(
        _outputs: &mut VoicesLayout<Self::OutputType>,
        _target: &VoiceTarget,
        _state: Self::VoiceState,
        _samples: usize,
    ) {
    }
}

pub struct RouterFactory<'f, 'c, D: RouterDataType, A: EngineAudioEnd> {
    pub(super) ctx: &'f mut ProcessContext<'c, A>,
    pub(super) module_id: ModuleId,
    pub(super) data_type: D,
}

impl<'f, 'c, A: EngineAudioEnd, D: RouterDataType> RouterFactory<'f, 'c, D, A> {
    pub fn params(&self) -> &ProcessParams<'_> {
        &self.ctx.params
    }

    pub fn for_channels(&mut self, mut f: impl FnMut(&mut Self, usize)) -> &mut Self {
        if !self.ctx.params.trigger_stage {
            for channel_idx in 0..NUM_CHANNELS {
                f(self, channel_idx);
            }
        }

        self
    }
}

impl<'f, 'c, A: EngineAudioEnd> RouterFactory<'f, 'c, AudioRouterType, A> {
    pub fn for_voices(
        &mut self,
        mut f: impl FnMut(&mut Self, &VoiceTarget, &mut VoicesLayout<SamplesOutput>),
    ) -> &mut Self {
        if !self.ctx.params.trigger_stage {
            let mut slot = self.ctx.outputs_arena.samples[self.data_type.samples_slot]
                .slot
                .take()
                .expect("slot should be in place");

            // Untriggered first so triggered voices can steal already-advanced phase.
            let last_voice_idx = self.ctx.params.last_voice_idx;

            for triggered in [false, true] {
                for voice in self
                    .ctx
                    .params
                    .active_voices
                    .iter()
                    .filter(|voice| voice.triggered().is_some() == triggered)
                {
                    for channel_idx in 0..NUM_CHANNELS {
                        let target = VoiceTarget::new(channel_idx, voice, last_voice_idx);

                        f(self, &target, &mut slot);
                    }
                }
            }

            self.ctx.outputs_arena.samples[self.data_type.samples_slot]
                .slot
                .replace(slot);
        }

        self
    }

    /// Groups active voices into SIMD pairs `(0, 1)`, `(2, 3)`, ….
    /// `prepare` runs once per occupied lane. `process` runs the filter for that pair's block.
    /// Slot order inside the pair is fixed, so state can stay in those lanes.
    pub fn for_lanes<M>(
        &mut self,
        module: &mut M,
        mut prepare: impl FnMut(&mut M, &mut Self, &VoiceTarget, usize),
        mut process: impl FnMut(
            &mut M,
            &mut Self,
            &VoicesBlock,
            &mut VoicesLayout<SamplesOutput>,
        ),
    ) -> &mut Self {
        if !self.ctx.params.trigger_stage {
            let mut slot = self.ctx.outputs_arena.samples[self.data_type.samples_slot]
                .slot
                .take()
                .expect("slot should be in place");

            let last_voice_idx = self.ctx.params.last_voice_idx;
            let mut pairs: [VoicesBlock; VOICE_PAIRS] = std::array::from_fn(|pair_idx| VoicesBlock {
                pair_idx,
                voices: [None; VOICE_PAIR_LEN],
                last_voice_idx,
            });
            let mut occupied = [false; VOICE_PAIRS];

            for voice in self.ctx.params.active_voices {
                let voice_idx = voice.voice_idx();
                let pair_idx = voice_idx / VOICE_PAIR_LEN;
                let slot_idx = voice_idx % VOICE_PAIR_LEN;

                pairs[pair_idx].voices[slot_idx] = Some(*voice);
                occupied[pair_idx] = true;
            }

            for (pair, is_occupied) in pairs.iter().zip(occupied) {
                if !is_occupied {
                    continue;
                }

                for (voice_slot, voice) in pair.voices.iter().enumerate() {
                    let Some(voice) = voice else {
                        continue;
                    };

                    for channel_idx in 0..NUM_CHANNELS {
                        let target = VoiceTarget::new(channel_idx, voice, pair.last_voice_idx);
                        let lane_idx = voice_slot * NUM_CHANNELS + channel_idx;

                        prepare(module, self, &target, lane_idx);
                    }
                }

                process(module, self, pair, &mut slot);
            }

            self.ctx.outputs_arena.samples[self.data_type.samples_slot]
                .slot
                .replace(slot);
        }

        self
    }

    pub fn for_triggered_voices(
        &mut self,
        mut f: impl FnMut(&mut Self, &VoiceTarget),
    ) -> &mut Self {
        if self.ctx.params.trigger_stage {
            let last_voice_idx = self.ctx.params.last_voice_idx;

            for voice in self.ctx.params.active_voices {
                for channel_idx in 0..NUM_CHANNELS {
                    let target = VoiceTarget::new(channel_idx, voice, last_voice_idx);

                    f(self, &target);
                }
            }
        }

        self
    }

    fn audio_voice_state(&self, target: &VoiceTarget) -> AudioVoiceState {
        AudioVoiceState {
            // Audio is sample-aligned: no trigger → offset 0; else silence [0..offset].
            offset: target.triggered.unwrap_or(0),
        }
    }

    pub fn router<'voice>(
        &'voice mut self,
        target: &'voice VoiceTarget,
    ) -> VoiceRouter<'voice, 'f, 'c, AudioRouterType, A>
    where
        'f: 'voice,
    {
        let state = self.audio_voice_state(target);

        VoiceRouter {
            factory: self,
            target,
            state,
        }
    }

    pub fn for_triggered_voice<'voice>(
        &'voice mut self,
        target: &'voice VoiceTarget,
    ) -> VoiceRouter<'voice, 'f, 'c, AudioRouterType, A>
    where
        'f: 'voice,
    {
        self.router(target)
    }

    pub fn for_voice<'voice>(
        &'voice mut self,
        target: &'voice VoiceTarget,
        outputs: &'voice mut VoicesLayout<SamplesOutput>,
    ) -> (
        VoiceRouter<'voice, 'f, 'c, AudioRouterType, A>,
        VoiceOutput<'voice, AudioRouterType>,
    )
    where
        'f: 'voice,
    {
        let samples = self.params().samples;
        let state = self.audio_voice_state(target);

        if let Some(offset) = target.triggered {
            outputs.at_mut(target.channel_idx, target.voice_idx).buffer[..offset.min(samples)]
                .fill(0.0);
        }

        (
            VoiceRouter {
                factory: self,
                target,
                state,
            },
            VoiceOutput {
                outputs,
                target,
                state,
                samples,
            },
        )
    }
}

impl<'f, 'c, A: EngineAudioEnd> RouterFactory<'f, 'c, ControlRouterType, A> {
    pub fn for_voices(
        &mut self,
        mut f: impl FnMut(&mut Self, &VoiceTarget, &mut VoicesLayout<SamplesOutput>),
    ) -> &mut Self {
        let mut slot = self.ctx.outputs_arena.samples[self.data_type.samples_slot]
            .slot
            .take()
            .expect("slot should be in place");

        let last_voice_idx = self.ctx.params.last_voice_idx;

        for channel_idx in 0..NUM_CHANNELS {
            for voice in self.ctx.params.active_voices {
                if !self.ctx.params.trigger_stage && voice.triggered().is_some() {
                    continue;
                }

                let target = VoiceTarget::new(channel_idx, voice, last_voice_idx);

                f(self, &target, &mut slot);
            }
        }

        self.ctx.outputs_arena.samples[self.data_type.samples_slot]
            .slot
            .replace(slot);

        self
    }

    pub fn for_voice<'voice>(
        &'voice mut self,
        target: &'voice VoiceTarget,
        outputs: &'voice mut VoicesLayout<SamplesOutput>,
    ) -> (
        VoiceRouter<'voice, 'f, 'c, ControlRouterType, A>,
        VoiceOutput<'voice, ControlRouterType>,
    )
    where
        'f: 'voice,
    {
        let triggered = target.triggered;
        let samples = self.params().samples;
        let output = outputs.at_mut(target.channel_idx, target.voice_idx);
        // Control runs 1 sample ahead: no trigger → offset 1 (seed buffer[0]);
        // Some(0) is a real note-on at sample 0, distinct from the non-trigger case.
        let state = ControlVoiceState {
            offset: triggered.unwrap_or(1),
        };

        if let Some(offset) = triggered {
            output.buffer[..offset.min(samples)].fill(0.0);
        } else {
            output.buffer[0] = output.next_frame_sample;
        }

        (
            VoiceRouter {
                factory: self,
                target,
                state,
            },
            VoiceOutput {
                outputs,
                target,
                state,
                samples,
            },
        )
    }
}

impl<'f, 'c, A: EngineAudioEnd> RouterFactory<'f, 'c, SpectralRouterType, A> {
    pub fn for_voices(
        &mut self,
        mut f: impl FnMut(&mut Self, &VoiceTarget, &mut VoicesLayout<SpectralOutput>),
    ) -> &mut Self {
        let mut slot = self.ctx.outputs_arena.spectral[self.data_type.spectral_slot]
            .slot
            .take()
            .expect("slot should be in place");

        let last_voice_idx = self.ctx.params.last_voice_idx;

        for channel_idx in 0..NUM_CHANNELS {
            for voice in self.ctx.params.active_voices {
                let target = VoiceTarget::new(channel_idx, voice, last_voice_idx);

                f(self, &target, &mut slot);
            }
        }

        self.ctx.outputs_arena.spectral[self.data_type.spectral_slot]
            .slot
            .replace(slot);

        self
    }

    pub fn for_voice<'voice>(
        &'voice mut self,
        target: &'voice VoiceTarget,
        outputs: &'voice mut VoicesLayout<SpectralOutput>,
    ) -> (
        VoiceRouter<'voice, 'f, 'c, SpectralRouterType, A>,
        VoiceOutput<'voice, SpectralRouterType>,
    )
    where
        'f: 'voice,
    {
        let samples = self.params().samples;
        // Spectral is block-rate: trigger_stage selects this-frame vs next-frame
        // control scalars; sample offset does not slice the spectrum.
        let state = SpectralVoiceState;

        (
            VoiceRouter {
                factory: self,
                target,
                state,
            },
            VoiceOutput {
                outputs,
                target,
                state,
                samples,
            },
        )
    }
}

impl<'f, 'c, A: EngineAudioEnd> RouterFactory<'f, 'c, OutputRouterType, A> {
    pub fn for_voice<'voice>(
        &'voice mut self,
        target: &'voice VoiceTarget,
    ) -> VoiceRouter<'voice, 'f, 'c, OutputRouterType, A>
    where
        'f: 'voice,
    {
        VoiceRouter {
            factory: self,
            target,
            state: OutputVoiceState,
        }
    }
}

pub struct VoiceRouter<'v, 'f, 'c, D: RouterDataType, A: EngineAudioEnd> {
    factory: &'v mut RouterFactory<'f, 'c, D, A>,
    target: &'v VoiceTarget,
    state: D::VoiceState,
}

pub struct VoiceOutput<'v, D: RouterDataType> {
    outputs: &'v mut VoicesLayout<D::OutputType>,
    target: &'v VoiceTarget,
    state: D::VoiceState,
    samples: usize,
}

impl<'v, 'f, 'c, A: EngineAudioEnd, D: RouterDataType> VoiceRouter<'v, 'f, 'c, D, A> {
    pub fn sample_rate(&self) -> Sample {
        self.factory.ctx.params.sample_rate
    }

    pub fn need_update_ui(&self) -> bool {
        self.target.is_last && self.factory.params().needs_update_ui
    }

    pub fn need_update_ui_mono(&self) -> bool {
        self.target.is_last && self.target.channel_idx == 0 && self.factory.params().needs_update_ui
    }

    fn scalar_param_impl(
        &mut self,
        input: &MixedSlots,
        param: Sample,
        this_frame: Option<usize>,
    ) -> Sample {
        if let Some(modulated_amount) = self.factory.ctx.outputs_arena.get_scalar(
            &input.slots,
            self.target.channel_idx,
            self.target.voice_idx,
            this_frame,
        ) {
            let value = param + modulated_amount;

            if self.need_update_ui() {
                self.factory.ctx.audio_end.update_modulated_input(
                    self.factory.module_id,
                    input.input_type,
                    self.target.channel_idx as u8,
                    value,
                    input.normalized_modulated(self.target.channel_idx, modulated_amount),
                );
            }

            value
        } else {
            param
        }
    }

    pub fn spectral(&self, slot: Option<usize>) -> &[ComplexSample] {
        self.factory
            .ctx
            .outputs_arena
            .get_spectral(slot, self.target.channel_idx, self.target.voice_idx)
            .unwrap_or(&[])
    }
}

/// One lane of a voice, placed at the trigger offset.
pub struct VoiceLane {
    lane_idx: usize,
    offset: usize,
    samples: usize,
}

impl VoiceLane {
    /// Active samples of this lane in `buffers`. Samples before the trigger offset are cleared.
    pub fn buff<'a>(&self, buffers: &'a mut [Buffer]) -> &'a mut [Sample] {
        let buffer = &mut buffers[self.lane_idx];

        buffer[..self.offset].fill(0.0);
        &mut buffer[self.offset..self.offset + self.samples]
    }
}

impl<'v, 'f, 'c, A: EngineAudioEnd> VoiceRouter<'v, 'f, 'c, AudioRouterType, A> {
    pub fn lane(&self, lane_idx: usize) -> VoiceLane {
        VoiceLane {
            lane_idx,
            offset: self.state.offset,
            samples: self.samples(),
        }
    }

    pub fn samples(&self) -> usize {
        self.factory.ctx.params.samples - self.state.offset
    }

    pub fn triggered(&self) -> bool {
        self.target.triggered.is_some()
    }

    pub fn direct(&self, slot: Option<usize>) -> &[Sample] {
        let ctx = &self.factory.ctx;

        &ctx.outputs_arena
            .get_buff(slot, self.target.channel_idx, self.target.voice_idx)
            .unwrap_or(&ZEROES_BUFFER)[self.state.offset..ctx.params.samples]
    }

    pub fn param(&mut self, input: &MixedSlots, param: &SmoothedSample, buff: &mut [Sample]) {
        let buff = &mut buff[..self.samples()];
        let smooth_params = &self.factory.ctx.params.smooth_params;

        if param.check_needs_smoothing(smooth_params) {
            param.smoothed_buff(buff, smooth_params);
        } else {
            buff.fill(param.get());
        }

        if self.factory.ctx.outputs_arena.add_buff_to(
            &input.slots,
            false,
            self.target.channel_idx,
            self.target.voice_idx,
            self.state.offset,
            buff,
        ) && self.need_update_ui()
        {
            let value = buff[0];

            self.factory.ctx.audio_end.update_modulated_input(
                self.factory.module_id,
                input.input_type,
                self.target.channel_idx as u8,
                value,
                input.normalized_modulated(self.target.channel_idx, value - param.get()),
            );
        }
    }

    pub fn scalar(&mut self, input: &MixedSlots, param: Sample, this_frame: bool) -> Sample {
        self.scalar_param_impl(input, param, this_frame.then_some(self.state.offset))
    }

    pub fn param_stationary(&self, input: &MixedSlots, param: &SmoothedSample) -> Option<Sample> {
        (!param.check_needs_smoothing(&self.factory.ctx.params.smooth_params) && input.is_empty())
            .then(|| param.get())
    }
}

impl<'v> VoiceOutput<'v, AudioRouterType> {
    pub fn output(&mut self) -> &mut [Sample] {
        &mut self
            .outputs
            .at_mut(self.target.channel_idx, self.target.voice_idx)
            .buffer[self.state.offset..self.samples]
    }
}

impl<'v, 'f, 'c, A: EngineAudioEnd> VoiceRouter<'v, 'f, 'c, ControlRouterType, A> {
    fn samples(&self) -> usize {
        self.factory.ctx.params.samples - self.state.offset + 1
    }

    pub fn triggered(&self) -> bool {
        self.target.triggered.is_some()
    }

    /// Maps an in-block sample offset to an index into [`VoiceOutput::output`].
    pub fn block_to_voice_offset(&self, offset: usize) -> usize {
        offset.saturating_sub(self.state.offset)
    }

    pub fn param(&mut self, input: &MixedSlots, param: &SmoothedSample, buff: &mut Buffer) {
        let buff = &mut buff[..self.samples()];
        let smooth_params = &self.factory.ctx.params.smooth_params;

        if param.check_needs_smoothing(smooth_params) {
            param.smoothed_buff(buff, smooth_params);
        } else {
            buff.fill(param.get());
        }

        if self.factory.ctx.outputs_arena.add_buff_to(
            &input.slots,
            true,
            self.target.channel_idx,
            self.target.voice_idx,
            self.state.offset,
            buff,
        ) && self.need_update_ui()
        {
            let value = buff[0];

            self.factory.ctx.audio_end.update_modulated_input(
                self.factory.module_id,
                input.input_type,
                self.target.channel_idx as u8,
                value,
                input.normalized_modulated(self.target.channel_idx, value - param.get()),
            );
        }
    }

    pub fn scalar(&mut self, input: &MixedSlots, param: Sample) -> Sample {
        self.scalar_param_impl(input, param, Some(self.state.offset))
    }
}

impl<'v> VoiceOutput<'v, ControlRouterType> {
    pub fn output(&mut self) -> &mut [Sample] {
        &mut self
            .outputs
            .at_mut(self.target.channel_idx, self.target.voice_idx)
            .buffer[self.state.offset..self.samples + 1]
    }

    /// External control sources are not written 1 sample ahead.
    /// in_buff is aligned with processing block.
    pub fn fill_with_ext_control(&mut self, in_buff: &[Sample]) {
        let offset = if self.target.triggered.is_some() {
            self.state.offset
        } else {
            0
        };
        let in_buff = &in_buff[offset.min(in_buff.len())..];
        let len = in_buff.len();
        let last = in_buff[len - 1];
        let output = self
            .outputs
            .at_mut(self.target.channel_idx, self.target.voice_idx);

        output.buffer[offset..offset + len].copy_from_slice(in_buff);
        output.buffer[offset + len] = last;
        output.next_frame_sample = last;
    }
}

impl<'v, 'f, 'c, A: EngineAudioEnd> VoiceRouter<'v, 'f, 'c, SpectralRouterType, A> {
    pub fn direct_opt(&self, slot: Option<usize>) -> Option<Sample> {
        let this_frame = self
            .factory
            .params()
            .trigger_stage
            .then_some(self.target.triggered)
            .flatten();

        slot.map(|slot| {
            self.factory.ctx.outputs_arena.control_scalar(
                slot,
                self.target.channel_idx,
                self.target.voice_idx,
                this_frame,
            )
        })
    }

    pub fn scalar(&mut self, input: &MixedSlots, param: Sample) -> Sample {
        let this_frame = self
            .factory
            .params()
            .trigger_stage
            .then_some(self.target.triggered)
            .flatten();

        self.scalar_param_impl(input, param, this_frame)
    }
}

impl<'v> VoiceOutput<'v, SpectralRouterType> {
    pub fn output(&mut self, length: usize) -> &mut [ComplexSample] {
        let out = self
            .outputs
            .at_mut(self.target.channel_idx, self.target.voice_idx);

        out.set_length(length);
        out.get_mut()
    }
}

impl<'v, D: RouterDataType> Drop for VoiceOutput<'v, D> {
    fn drop(&mut self) {
        D::advance(self.outputs, self.target, self.state, self.samples);
    }
}

impl<'v, 'f, 'c, A: EngineAudioEnd> VoiceRouter<'v, 'f, 'c, OutputRouterType, A> {
    pub fn direct(&mut self, slot: Option<usize>) -> &[Sample] {
        let ctx = &self.factory.ctx;

        &ctx.outputs_arena
            .get_buff(slot, self.target.channel_idx, self.target.voice_idx)
            .unwrap_or(&ZEROES_BUFFER)[..ctx.params.samples]
    }
}

#[cfg(test)]
mod tests {
    use super::VoiceLane;
    use crate::synth_engine::buffer::{BUFFER_SIZE, zero_buffer};

    #[test]
    fn lane_clears_samples_before_trigger() {
        let lane = VoiceLane {
            lane_idx: 2,
            offset: 3,
            samples: 4,
        };
        let mut buffers = [zero_buffer(); 4];

        buffers[2].fill(1.0);

        let active = lane.buff(&mut buffers);
        let active = active.to_vec();

        assert_eq!(active, [1.0; 4]);
        assert_eq!(&buffers[2][..3], &[0.0; 3]);
        assert_eq!(buffers[2][3], 1.0);
        assert_eq!(buffers[2][7], 1.0);
        assert_eq!(buffers[1][0], 0.0);
        assert_eq!(buffers[2].len(), BUFFER_SIZE);
    }
}
