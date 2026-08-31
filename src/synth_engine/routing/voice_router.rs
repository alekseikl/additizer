use crate::synth_engine::{
    Buffer, ComplexSample, ModuleId, NUM_CHANNELS, ProcessParams, Sample,
    buffer::{VoicesLayout, ZEROES_BUFFER, ZEROES_SPECTRAL_BUFFER},
    routing::{
        InputSlots, ProcessContext, SamplesOutput, SpectralOutput, process_context::VoiceTarget,
    },
    smooth::SmoothedSample,
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
    bandwidth: usize,
}

pub struct AudioRouterType {
    pub(super) samples_slot: usize,
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
        let output = &mut outputs[target.channel_idx][target.voice_idx];

        output.next_frame_sample = output.buffer[samples];
    }
}

#[derive(Clone, Copy)]
pub struct SpectralVoiceState {
    bandwidth: usize,
}

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

pub struct RouterFactory<'f, 'c, D: RouterDataType> {
    pub(super) ctx: &'f mut ProcessContext<'c>,
    pub(super) module_id: ModuleId,
    pub(super) data_type: D,
}

impl<'f, 'c, D: RouterDataType> RouterFactory<'f, 'c, D> {
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

    fn bandwidth(&self, note_bandwidth: usize) -> usize {
        let bandwidth = self.params().bandwidth;
        let bandwidth = if bandwidth == 0 {
            note_bandwidth
        } else {
            bandwidth
        };

        bandwidth + 1 // Add DC
    }
}

impl<'f, 'c> RouterFactory<'f, 'c, AudioRouterType> {
    fn visit_voices(
        &mut self,
        mut f: impl FnMut(&mut Self, &VoiceTarget, &mut VoicesLayout<SamplesOutput>),
    ) {
        let mut slot = self.ctx.outputs_arena.samples[self.data_type.samples_slot]
            .slot
            .take()
            .expect("slot should be in place");

        for channel_idx in 0..NUM_CHANNELS {
            for (seq_idx, voice) in self.ctx.params.active_voices.iter().enumerate() {
                let target = VoiceTarget::new(channel_idx, voice, seq_idx);

                f(self, &target, &mut slot);
            }
        }

        self.ctx.outputs_arena.samples[self.data_type.samples_slot]
            .slot
            .replace(slot);
    }

    pub fn for_voices(
        &mut self,
        f: impl FnMut(&mut Self, &VoiceTarget, &mut VoicesLayout<SamplesOutput>),
    ) -> &mut Self {
        if !self.ctx.params.trigger_stage {
            self.visit_voices(f);
        }

        self
    }

    pub fn for_triggered_voices(
        &mut self,
        f: impl FnMut(&mut Self, &VoiceTarget, &mut VoicesLayout<SamplesOutput>),
    ) -> &mut Self {
        if self.ctx.params.trigger_stage {
            self.visit_voices(f);
        }

        self
    }

    pub fn for_voice<'voice>(
        &'voice mut self,
        target: &'voice VoiceTarget,
        outputs: &'voice mut VoicesLayout<SamplesOutput>,
    ) -> (
        VoiceRouter<'voice, 'f, 'c, AudioRouterType>,
        VoiceOutput<'voice, AudioRouterType>,
    )
    where
        'f: 'voice,
    {
        let triggered = target.triggered;
        let samples = self.params().samples;
        // Audio is sample-aligned: no trigger → offset 0; else silence [0..offset].
        let state = AudioVoiceState {
            offset: triggered.unwrap_or(0),
            bandwidth: self.bandwidth(target.note_bandwidth),
        };

        if let Some(offset) = triggered {
            outputs[target.channel_idx][target.voice_idx].buffer[..offset.min(samples)].fill(0.0);
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

impl<'f, 'c> RouterFactory<'f, 'c, ControlRouterType> {
    pub fn for_voices(
        &mut self,
        mut f: impl FnMut(&mut Self, &VoiceTarget, &mut VoicesLayout<SamplesOutput>),
    ) -> &mut Self {
        let mut slot = self.ctx.outputs_arena.samples[self.data_type.samples_slot]
            .slot
            .take()
            .expect("slot should be in place");

        for channel_idx in 0..NUM_CHANNELS {
            for (seq_idx, voice) in self.ctx.params.active_voices.iter().enumerate() {
                if !self.ctx.params.trigger_stage && voice.triggered().is_some() {
                    continue;
                }

                let target = VoiceTarget::new(channel_idx, voice, seq_idx);

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
        VoiceRouter<'voice, 'f, 'c, ControlRouterType>,
        VoiceOutput<'voice, ControlRouterType>,
    )
    where
        'f: 'voice,
    {
        let triggered = target.triggered;
        let samples = self.params().samples;
        let output = &mut outputs[target.channel_idx][target.voice_idx];
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

impl<'f, 'c> RouterFactory<'f, 'c, SpectralRouterType> {
    pub fn for_voices(
        &mut self,
        mut f: impl FnMut(&mut Self, &VoiceTarget, &mut VoicesLayout<SpectralOutput>),
    ) -> &mut Self {
        let mut slot = self.ctx.outputs_arena.spectral[self.data_type.spectral_slot]
            .slot
            .take()
            .expect("slot should be in place");

        for channel_idx in 0..self.ctx.params.spectrum_channels {
            for (seq_idx, voice) in self.ctx.params.active_voices.iter().enumerate() {
                let target = VoiceTarget::new(channel_idx, voice, seq_idx);

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
        VoiceRouter<'voice, 'f, 'c, SpectralRouterType>,
        VoiceOutput<'voice, SpectralRouterType>,
    )
    where
        'f: 'voice,
    {
        let samples = self.params().samples;
        // Spectral is block-rate: trigger_stage selects this-frame vs next-frame
        // control scalars; sample offset does not slice the spectrum.
        let state = SpectralVoiceState {
            bandwidth: self.bandwidth(target.note_bandwidth),
        };

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

impl<'f, 'c> RouterFactory<'f, 'c, OutputRouterType> {
    pub fn for_voice<'voice>(
        &'voice mut self,
        target: &'voice VoiceTarget,
    ) -> VoiceRouter<'voice, 'f, 'c, OutputRouterType>
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

pub struct VoiceRouter<'v, 'f, 'c, D: RouterDataType> {
    factory: &'v mut RouterFactory<'f, 'c, D>,
    target: &'v VoiceTarget,
    state: D::VoiceState,
}

pub struct VoiceOutput<'v, D: RouterDataType> {
    outputs: &'v mut VoicesLayout<D::OutputType>,
    target: &'v VoiceTarget,
    state: D::VoiceState,
    samples: usize,
}

impl<'v, 'f, 'c, D: RouterDataType> VoiceRouter<'v, 'f, 'c, D> {
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
        input: &InputSlots,
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

    fn spectral_impl(&self, slot: Option<usize>, bandwidth: usize) -> &[ComplexSample] {
        let buff = self
            .factory
            .ctx
            .outputs_arena
            .get_spectral(slot, self.target.channel_idx, self.target.voice_idx)
            .unwrap_or(&ZEROES_SPECTRAL_BUFFER);

        &buff[..buff.len().min(bandwidth)]
    }
}

impl<'v, 'f, 'c> VoiceRouter<'v, 'f, 'c, AudioRouterType> {
    pub fn samples(&self) -> usize {
        self.factory.ctx.params.samples - self.state.offset
    }

    pub fn triggered(&self) -> bool {
        self.target.triggered.is_some()
    }

    pub fn direct(&mut self, slot: Option<usize>) -> &[Sample] {
        let ctx = &self.factory.ctx;

        &ctx.outputs_arena
            .get_buff(slot, self.target.channel_idx, self.target.voice_idx)
            .unwrap_or(&ZEROES_BUFFER)[self.state.offset..ctx.params.samples]
    }

    pub fn param(&mut self, input: &InputSlots, param: &SmoothedSample, buff: &mut Buffer) {
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

    pub fn scalar(&mut self, input: &InputSlots, param: Sample, this_frame: bool) -> Sample {
        self.scalar_param_impl(input, param, this_frame.then_some(self.state.offset))
    }

    pub fn spectral(&self, slot: Option<usize>) -> &[ComplexSample] {
        self.spectral_impl(slot, self.state.bandwidth)
    }

    pub fn param_stationary_at(
        &self,
        input: &InputSlots,
        param: &SmoothedSample,
        value: Sample,
    ) -> bool {
        !param.check_needs_smoothing(&self.factory.ctx.params.smooth_params)
            && (param.get() - value).abs() < 1e-6
            && input.is_empty()
    }
}

impl<'v> VoiceOutput<'v, AudioRouterType> {
    pub fn output(&mut self) -> &mut [Sample] {
        &mut self.outputs[self.target.channel_idx][self.target.voice_idx].buffer
            [self.state.offset..self.samples]
    }
}

impl<'v, 'f, 'c> VoiceRouter<'v, 'f, 'c, ControlRouterType> {
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

    pub fn param(&mut self, input: &InputSlots, param: &SmoothedSample, buff: &mut Buffer) {
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
        ) {
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

    pub fn scalar(&mut self, input: &InputSlots, param: Sample) -> Sample {
        self.scalar_param_impl(input, param, Some(self.state.offset))
    }
}

impl<'v> VoiceOutput<'v, ControlRouterType> {
    pub fn output(&mut self) -> &mut [Sample] {
        &mut self.outputs[self.target.channel_idx][self.target.voice_idx].buffer
            [self.state.offset..self.samples + 1]
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
        let output = &mut self.outputs[self.target.channel_idx][self.target.voice_idx];

        output.buffer[offset..offset + len].copy_from_slice(in_buff);
        output.buffer[offset + len] = last;
        output.next_frame_sample = last;
    }
}

impl<'v, 'f, 'c> VoiceRouter<'v, 'f, 'c, SpectralRouterType> {
    pub fn scalar(&mut self, input: &InputSlots, param: Sample) -> Sample {
        let this_frame = self
            .factory
            .params()
            .trigger_stage
            .then_some(self.target.triggered)
            .flatten();

        self.scalar_param_impl(input, param, this_frame)
    }

    pub fn spectral(&self, slot: Option<usize>) -> &[ComplexSample] {
        self.spectral_impl(slot, self.state.bandwidth)
    }
}

impl<'v> VoiceOutput<'v, SpectralRouterType> {
    pub fn output(&mut self) -> &mut [ComplexSample] {
        let buff = self.outputs[self.target.channel_idx][self.target.voice_idx].buff();
        let bandwidth = buff.len().min(self.state.bandwidth);

        &mut buff[..bandwidth]
    }
}

impl<'v, D: RouterDataType> Drop for VoiceOutput<'v, D> {
    fn drop(&mut self) {
        D::advance(self.outputs, self.target, self.state, self.samples);
    }
}

impl<'v, 'f, 'c> VoiceRouter<'v, 'f, 'c, OutputRouterType> {
    pub fn direct(&mut self, slot: Option<usize>) -> &[Sample] {
        let ctx = &self.factory.ctx;

        &ctx.outputs_arena
            .get_buff(slot, self.target.channel_idx, self.target.voice_idx)
            .unwrap_or(&ZEROES_BUFFER)[..ctx.params.samples]
    }
}
