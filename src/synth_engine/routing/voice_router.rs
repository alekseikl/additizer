use crate::synth_engine::{
    Buffer, ComplexSample, ModuleId, NUM_CHANNELS, ProcessParams, Sample,
    buffer::{VoicesLayout, ZEROES_BUFFER, ZEROES_SPECTRAL_BUFFER},
    routing::{
        InputSlots, ProcessContext, SamplesOutput, SpectralOutput, process_context::VoiceTarget,
    },
    smooth::SmoothedSample,
    voices_handler::PlayingVoice,
};

pub trait VoiceExt {
    fn advance(&mut self, channel_idx: usize, voice_idx: usize, samples: usize);
}

pub trait RouterDataType {
    type OutputType;
    type VoiceExt<'v>: VoiceExt;
}

pub struct AudioVoiceExt<'v> {
    offset: usize,
    bandwidth: usize,
    outputs: &'v mut VoicesLayout<SamplesOutput>,
    triggers: &'v mut VoicesLayout<Option<usize>>,
    triggered: bool,
}

impl<'v> VoiceExt for AudioVoiceExt<'v> {
    fn advance(&mut self, channel_idx: usize, voice_idx: usize, _samples: usize) {
        self.triggers[channel_idx][voice_idx] = None;
    }
}

pub struct AudioRouterType {
    pub(super) samples_slot: usize,
}

impl RouterDataType for AudioRouterType {
    type OutputType = SamplesOutput;
    type VoiceExt<'v> = AudioVoiceExt<'v>;
}

pub struct ControlVoiceExt<'v> {
    offset: usize,
    outputs: &'v mut VoicesLayout<SamplesOutput>,
    triggers: &'v mut VoicesLayout<Option<usize>>,
    triggered: bool,
}

impl<'v> VoiceExt for ControlVoiceExt<'v> {
    fn advance(&mut self, channel_idx: usize, voice_idx: usize, samples: usize) {
        let trigger = &mut self.triggers[channel_idx][voice_idx];
        let output = &mut self.outputs[channel_idx][voice_idx];

        output.next_frame_sample = output.buffer[samples];
        *trigger = None;
    }
}

pub struct ControlRouterType {
    pub(super) samples_slot: usize,
}

impl RouterDataType for ControlRouterType {
    type OutputType = SamplesOutput;
    type VoiceExt<'v> = ControlVoiceExt<'v>;
}

pub struct SpectralVoiceExt<'v> {
    bandwidth: usize,
    outputs: &'v mut VoicesLayout<SpectralOutput>,
    triggers: &'v mut VoicesLayout<Option<usize>>,
    triggered: bool,
}

impl<'v> VoiceExt for SpectralVoiceExt<'v> {
    fn advance(&mut self, channel_idx: usize, voice_idx: usize, _samples: usize) {
        self.outputs[channel_idx][voice_idx].advance();
        self.triggers[channel_idx][voice_idx] = None;
    }
}

pub struct SpectralRouterType {
    pub(super) spectral_slot: usize,
}

impl RouterDataType for SpectralRouterType {
    type OutputType = SpectralOutput;
    type VoiceExt<'v> = SpectralVoiceExt<'v>;
}

pub struct OutputVoiceExt {}

impl VoiceExt for OutputVoiceExt {
    fn advance(&mut self, _channel_idx: usize, _voice_idx: usize, _samples: usize) {}
}

pub struct OutputRouterType;

impl RouterDataType for OutputRouterType {
    type OutputType = SamplesOutput;
    type VoiceExt<'v> = OutputVoiceExt;
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

    fn bandwidth(&self, note_bandwidth: usize) -> usize {
        let bandwidth = self.params().bandwidth;
        let bandwidth = if bandwidth == 0 {
            note_bandwidth
        } else {
            bandwidth
        };

        bandwidth + 1 // Add DC
    }

    pub fn for_voice<'voice>(
        &'voice mut self,
        channel_idx: usize,
        playing_voice: PlayingVoice,
        seq_idx: usize,
    ) -> VoiceRouter<'voice, 'f, 'c, D>
    where
        'f: 'voice,
    {
        VoiceRouter {
            factory: self,
            channel_idx,
            playing_voice,
            seq_idx,
        }
    }
}

impl<'f, 'c> RouterFactory<'f, 'c, AudioRouterType> {
    pub fn with_output_slot(
        &mut self,
        f: impl FnOnce(&mut Self, &mut VoicesLayout<SamplesOutput>),
    ) {
        let mut slot = self.ctx.outputs_arena.samples[self.data_type.samples_slot]
            .slot
            .take()
            .expect("slot should be in place");

        f(self, &mut slot);

        self.ctx.outputs_arena.samples[self.data_type.samples_slot]
            .slot
            .replace(slot);
    }

    pub fn with_output_slot2(
        &mut self,
        mut f: impl FnMut(&mut Self, VoiceTarget, &mut VoicesLayout<SamplesOutput>),
    ) {
        let mut slot = self.ctx.outputs_arena.samples[self.data_type.samples_slot]
            .slot
            .take()
            .expect("slot should be in place");

        for channel_idx in 0..NUM_CHANNELS {
            for (seq_idx, playing) in self.ctx.params.active_voices.iter().enumerate() {
                let target = VoiceTarget {
                    channel_idx,
                    voice_idx: playing.voice_idx(),
                    note_bandwidth: playing.note_bandwidth(),
                    is_last: seq_idx == 0,
                };

                f(self, target, &mut slot);
            }
        }

        self.ctx.outputs_arena.samples[self.data_type.samples_slot]
            .slot
            .replace(slot);
    }

    pub fn for_voice2<'voice>(
        &'voice mut self,
        target: VoiceTarget,
        triggers: &'voice mut VoicesLayout<Option<usize>>,
        outputs: &'voice mut VoicesLayout<SamplesOutput>,
    ) -> VoiceRouter2<'voice, 'f, 'c, AudioRouterType>
    where
        'f: 'voice,
    {
        let triggered = triggers[target.channel_idx][target.voice_idx];
        let bandwidth = self.bandwidth(target.note_bandwidth);

        if let Some(offset) = triggered {
            outputs[target.channel_idx][target.voice_idx].output(self.params().samples)[..offset]
                .fill(0.0);
        }

        VoiceRouter2 {
            factory: self,
            target,
            ext: AudioVoiceExt {
                offset: triggered.unwrap_or(0),
                bandwidth,
                outputs,
                triggers,
                triggered: triggered.is_some(),
            },
        }
    }
}

impl<'f, 'c> RouterFactory<'f, 'c, ControlRouterType> {
    pub fn with_output_slot(
        &mut self,
        f: impl FnOnce(&mut Self, &mut VoicesLayout<SamplesOutput>),
    ) {
        let mut slot = self.ctx.outputs_arena.samples[self.data_type.samples_slot]
            .slot
            .take()
            .expect("slot should be in place");

        f(self, &mut slot);

        self.ctx.outputs_arena.samples[self.data_type.samples_slot]
            .slot
            .replace(slot);
    }

    pub fn with_output_slot2(
        &mut self,
        mut f: impl FnMut(&mut Self, VoiceTarget, &mut VoicesLayout<SamplesOutput>),
    ) {
        let mut slot = self.ctx.outputs_arena.samples[self.data_type.samples_slot]
            .slot
            .take()
            .expect("slot should be in place");

        for channel_idx in 0..NUM_CHANNELS {
            for (seq_idx, playing) in self.ctx.params.active_voices.iter().enumerate() {
                let target = VoiceTarget {
                    channel_idx,
                    voice_idx: playing.voice_idx(),
                    note_bandwidth: playing.note_bandwidth(),
                    is_last: seq_idx == 0,
                };

                f(self, target, &mut slot);
            }
        }

        self.ctx.outputs_arena.samples[self.data_type.samples_slot]
            .slot
            .replace(slot);
    }

    pub fn for_voice2<'voice>(
        &'voice mut self,
        target: VoiceTarget,
        triggers: &'voice mut VoicesLayout<Option<usize>>,
        outputs: &'voice mut VoicesLayout<SamplesOutput>,
    ) -> VoiceRouter2<'voice, 'f, 'c, ControlRouterType>
    where
        'f: 'voice,
    {
        let triggered = triggers[target.channel_idx][target.voice_idx];
        let output = &mut outputs[target.channel_idx][target.voice_idx];

        if let Some(offset) = triggered {
            output.buffer[..offset.min(self.params().samples)].fill(0.0);
        } else {
            output.buffer[0] = output.next_frame_sample;
        }

        VoiceRouter2 {
            factory: self,
            target,
            ext: ControlVoiceExt {
                offset: triggered.unwrap_or(1),
                outputs,
                triggers,
                triggered: triggered.is_some(),
            },
        }
    }
}

impl<'f, 'c> RouterFactory<'f, 'c, SpectralRouterType> {
    pub fn with_output_slot(
        &mut self,
        f: impl FnOnce(&mut Self, &mut VoicesLayout<SpectralOutput>),
    ) {
        let mut slot = self.ctx.outputs_arena.spectral[self.data_type.spectral_slot]
            .slot
            .take()
            .expect("slot should be in place");

        f(self, &mut slot);

        self.ctx.outputs_arena.spectral[self.data_type.spectral_slot]
            .slot
            .replace(slot);
    }

    pub fn with_output_slot2(
        &mut self,
        mut f: impl FnMut(&mut Self, VoiceTarget, &mut VoicesLayout<SpectralOutput>) -> bool,
    ) {
        let mut slot = self.ctx.outputs_arena.spectral[self.data_type.spectral_slot]
            .slot
            .take()
            .expect("slot should be in place");

        for channel_idx in 0..NUM_CHANNELS {
            for (seq_idx, playing) in self.ctx.params.active_voices.iter().enumerate() {
                let target = VoiceTarget {
                    channel_idx,
                    voice_idx: playing.voice_idx(),
                    note_bandwidth: playing.note_bandwidth(),
                    is_last: seq_idx == 0,
                };

                if f(self, target, &mut slot) {
                    f(self, target, &mut slot);
                }
            }
        }

        self.ctx.outputs_arena.spectral[self.data_type.spectral_slot]
            .slot
            .replace(slot);
    }

    pub fn for_voice2<'voice>(
        &'voice mut self,
        target: VoiceTarget,
        triggers: &'voice mut VoicesLayout<Option<usize>>,
        outputs: &'voice mut VoicesLayout<SpectralOutput>,
    ) -> VoiceRouter2<'voice, 'f, 'c, SpectralRouterType>
    where
        'f: 'voice,
    {
        let triggered = triggers[target.channel_idx][target.voice_idx];
        let bandwidth = self.bandwidth(target.note_bandwidth);

        VoiceRouter2 {
            factory: self,
            target,
            ext: SpectralVoiceExt {
                bandwidth,
                outputs,
                triggers,
                triggered: triggered.is_some(),
            },
        }
    }
}

impl<'f, 'c> RouterFactory<'f, 'c, OutputRouterType> {
    pub fn for_voice2<'voice>(
        &'voice mut self,
        target: VoiceTarget,
    ) -> VoiceRouter2<'voice, 'f, 'c, OutputRouterType>
    where
        'f: 'voice,
    {
        VoiceRouter2 {
            factory: self,
            target,
            ext: OutputVoiceExt {},
        }
    }
}

pub struct VoiceRouter<'v, 'f, 'c, S: RouterDataType> {
    factory: &'v mut RouterFactory<'f, 'c, S>,
    channel_idx: usize,
    playing_voice: PlayingVoice,
    seq_idx: usize,
}

impl<'v, 'f, 'c, S: RouterDataType> VoiceRouter<'v, 'f, 'c, S> {
    pub fn samples(&self) -> usize {
        self.factory.ctx.params.samples
    }

    pub fn sample_rate(&self) -> Sample {
        self.factory.ctx.params.sample_rate
    }

    pub fn channel_idx(&self) -> usize {
        self.channel_idx
    }

    pub fn voice_idx(&self) -> usize {
        self.playing_voice.voice_idx()
    }

    pub fn need_update_ui(&self) -> bool {
        self.seq_idx == 0 && self.factory.params().needs_update_ui
    }

    pub fn need_update_ui_mono(&self) -> bool {
        self.seq_idx == 0 && self.channel_idx == 0 && self.factory.params().needs_update_ui
    }

    fn buff_impl(&mut self, slot: Option<usize>) -> &[Sample] {
        self.factory
            .ctx
            .outputs_arena
            .get_buff(slot, self.channel_idx, self.voice_idx())
            .unwrap_or(&ZEROES_BUFFER)
    }

    fn scalar_param_impl(&mut self, input: &InputSlots, param: Sample, triggered: bool) -> Sample {
        if let Some(modulated_amount) = self.factory.ctx.outputs_arena.get_scalar(
            &input.slots,
            self.channel_idx,
            self.voice_idx(),
            triggered.then_some(0),
        ) {
            let value = param + modulated_amount;

            if self.need_update_ui() {
                self.factory.ctx.audio_end.update_modulated_input(
                    self.factory.module_id,
                    input.input_type,
                    self.channel_idx as u8,
                    value,
                    input.normalized_modulated(self.channel_idx, modulated_amount),
                );
            }

            value
        } else {
            param
        }
    }

    fn spectral_impl(&self, slot: Option<usize>, triggered: bool) -> &[ComplexSample] {
        let buff = self
            .factory
            .ctx
            .outputs_arena
            .get_spectral(slot, self.channel_idx, self.voice_idx(), triggered)
            .unwrap_or(&ZEROES_SPECTRAL_BUFFER);

        let bandwidth = self.factory.params().bandwidth;

        let bandwidth = if bandwidth == 0 {
            self.playing_voice.note_bandwidth()
        } else {
            bandwidth
        } + 1; // Add DC

        &buff[..buff.len().min(bandwidth)]
    }
}

impl<'v, 'f, 'c> VoiceRouter<'v, 'f, 'c, AudioRouterType> {
    pub fn buff(&mut self, slot: Option<usize>) -> &[Sample] {
        self.buff_impl(slot)
    }

    pub fn buff_param(
        &mut self,
        input: &InputSlots,
        param: &mut SmoothedSample,
        buff: &mut Buffer,
    ) {
        let params = &self.factory.ctx.params;
        let buff = &mut buff[..params.samples];

        if param.check_needs_smoothing(&params.smooth_params) {
            param.smoothed_buff(buff, &params.smooth_params);
        } else {
            buff.fill(param.get());
        }

        if self.factory.ctx.outputs_arena.add_buff_to(
            &input.slots,
            self.channel_idx,
            self.voice_idx(),
            0,
            buff,
        ) && self.need_update_ui()
        {
            let value = buff[0];

            self.factory.ctx.audio_end.update_modulated_input(
                self.factory.module_id,
                input.input_type,
                self.channel_idx as u8,
                value,
                input.normalized_modulated(self.channel_idx, value - param.get()),
            );
        }
    }

    pub fn scalar_param(&mut self, input: &InputSlots, param: Sample, triggered: bool) -> Sample {
        self.scalar_param_impl(input, param, triggered)
    }

    pub fn spectral(&self, slot: Option<usize>, triggered: bool) -> &[ComplexSample] {
        self.spectral_impl(slot, triggered)
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

impl<'v, 'f, 'c> VoiceRouter<'v, 'f, 'c, ControlRouterType> {
    pub fn buff_param(
        &mut self,
        input: &InputSlots,
        param: &mut SmoothedSample,
        buff: &mut Buffer,
        triggered: bool,
    ) {
        let skip = usize::from(!triggered);
        let params = &self.factory.ctx.params;
        let buff = &mut buff[..params.samples + 1 - skip];

        if param.check_needs_smoothing(&params.smooth_params) {
            param.smoothed_buff(buff, &params.smooth_params);
        } else {
            buff.fill(param.get());
        }

        if self.factory.ctx.outputs_arena.add_buff_to(
            &input.slots,
            self.channel_idx,
            self.voice_idx(),
            skip,
            buff,
        ) {
            let value = buff[0];

            self.factory.ctx.audio_end.update_modulated_input(
                self.factory.module_id,
                input.input_type,
                self.channel_idx as u8,
                value,
                input.normalized_modulated(self.channel_idx, value - param.get()),
            );
        }
    }

    pub fn scalar_param(&mut self, input: &InputSlots, param: Sample, triggered: bool) -> Sample {
        self.scalar_param_impl(input, param, triggered)
    }
}

impl<'v, 'f, 'c> VoiceRouter<'v, 'f, 'c, SpectralRouterType> {
    pub fn scalar_param(&mut self, input: &InputSlots, param: Sample, triggered: bool) -> Sample {
        self.scalar_param_impl(input, param, triggered)
    }

    pub fn spectral(&self, slot: Option<usize>, triggered: bool) -> &[ComplexSample] {
        self.spectral_impl(slot, triggered)
    }
}

impl<'v, 'f, 'c> VoiceRouter<'v, 'f, 'c, OutputRouterType> {
    pub fn buff(&mut self, slot: Option<usize>) -> &[Sample] {
        self.buff_impl(slot)
    }
}

//================================
pub struct VoiceRouter2<'v, 'f, 'c, D: RouterDataType> {
    factory: &'v mut RouterFactory<'f, 'c, D>,
    ext: D::VoiceExt<'v>,
    target: VoiceTarget,
}

impl<'v, 'f, 'c, D: RouterDataType> VoiceRouter2<'v, 'f, 'c, D> {
    pub fn sample_rate(&self) -> Sample {
        self.factory.ctx.params.sample_rate
    }

    pub fn channel_idx(&self) -> usize {
        self.target.channel_idx
    }

    pub fn voice_idx(&self) -> usize {
        self.target.voice_idx
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

    fn spectral_impl(
        &self,
        slot: Option<usize>,
        this_frame: bool,
        bandwidth: usize,
    ) -> &[ComplexSample] {
        let buff = self
            .factory
            .ctx
            .outputs_arena
            .get_spectral(
                slot,
                self.target.channel_idx,
                self.target.voice_idx,
                this_frame,
            )
            .unwrap_or(&ZEROES_SPECTRAL_BUFFER);

        &buff[..buff.len().min(bandwidth)]
    }
}

impl<'v, 'f, 'c> VoiceRouter2<'v, 'f, 'c, AudioRouterType> {
    pub fn samples(&self) -> usize {
        self.factory.ctx.params.samples - self.ext.offset
    }

    pub fn triggered(&self) -> bool {
        self.ext.triggered
    }

    fn direct(&mut self, slot: Option<usize>) -> &[Sample] {
        let ctx = &self.factory.ctx;

        &ctx.outputs_arena
            .get_buff(slot, self.target.channel_idx, self.target.voice_idx)
            .unwrap_or(&ZEROES_BUFFER)[self.ext.offset..ctx.params.samples]
    }

    pub fn param(&mut self, input: &InputSlots, param: &mut SmoothedSample, buff: &mut Buffer) {
        let buff = &mut buff[..self.samples()];
        let smooth_params = &self.factory.ctx.params.smooth_params;

        if param.check_needs_smoothing(smooth_params) {
            param.smoothed_buff(buff, smooth_params);
        } else {
            buff.fill(param.get());
        }

        if self.factory.ctx.outputs_arena.add_buff_to(
            &input.slots,
            self.target.channel_idx,
            self.target.voice_idx,
            self.ext.offset,
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
        self.scalar_param_impl(input, param, this_frame.then_some(self.ext.offset))
    }

    pub fn spectral(&self, slot: Option<usize>, this_frame: bool) -> &[ComplexSample] {
        self.spectral_impl(slot, this_frame, self.ext.bandwidth)
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

    pub fn output(&mut self) -> &mut [Sample] {
        &mut self.ext.outputs[self.target.channel_idx][self.target.voice_idx].buffer
            [self.ext.offset..self.factory.ctx.params.samples]
    }
}

impl<'v, 'f, 'c> VoiceRouter2<'v, 'f, 'c, ControlRouterType> {
    pub fn samples(&self) -> usize {
        self.factory.ctx.params.samples - self.ext.offset + 1
    }

    pub fn triggered(&self) -> bool {
        self.ext.triggered
    }

    pub fn param(&mut self, input: &InputSlots, param: &mut SmoothedSample, buff: &mut Buffer) {
        let buff = &mut buff[..self.samples()];
        let smooth_params = &self.factory.ctx.params.smooth_params;

        if param.check_needs_smoothing(smooth_params) {
            param.smoothed_buff(buff, smooth_params);
        } else {
            buff.fill(param.get());
        }

        if self.factory.ctx.outputs_arena.add_buff_to(
            &input.slots,
            self.target.channel_idx,
            self.target.voice_idx,
            self.ext.offset,
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
        self.scalar_param_impl(input, param, Some(self.ext.offset))
    }

    pub fn output(&mut self) -> &mut [Sample] {
        &mut self.ext.outputs[self.target.channel_idx][self.target.voice_idx].buffer
            [self.ext.offset..self.factory.ctx.params.samples + 1]
    }

    pub fn audio_output(&mut self) -> &mut [Sample] {
        let offset = if self.ext.triggered {
            self.ext.offset
        } else {
            0
        };

        &mut self.ext.outputs[self.target.channel_idx][self.target.voice_idx].buffer
            [offset..self.factory.ctx.params.samples]
    }
}

impl<'v, 'f, 'c> VoiceRouter2<'v, 'f, 'c, SpectralRouterType> {
    pub fn scalar(&mut self, input: &InputSlots, param: Sample) -> Sample {
        self.scalar_param_impl(input, param, self.ext.triggered.then_some(0))
    }

    pub fn spectral(&self, slot: Option<usize>) -> &[ComplexSample] {
        self.spectral_impl(slot, self.ext.triggered, self.ext.bandwidth)
    }

    pub fn output(&mut self) -> &mut [ComplexSample] {
        let buff = self.ext.outputs[self.target.channel_idx][self.target.voice_idx].buff();
        let bandwidth = buff.len().min(self.ext.bandwidth);

        &mut buff[..bandwidth]
    }
}

impl<'v, 'f, 'c> VoiceRouter2<'v, 'f, 'c, OutputRouterType> {
    pub fn samples(&self) -> usize {
        self.factory.ctx.params.samples
    }

    fn direct(&mut self, slot: Option<usize>) -> &[Sample] {
        let ctx = &self.factory.ctx;

        &ctx.outputs_arena
            .get_buff(slot, self.target.channel_idx, self.target.voice_idx)
            .unwrap_or(&ZEROES_BUFFER)[..ctx.params.samples]
    }
}

impl<'v, 'f, 'c, S: RouterDataType> Drop for VoiceRouter2<'v, 'f, 'c, S> {
    fn drop(&mut self) {
        self.ext.advance(
            self.target.channel_idx,
            self.target.voice_idx,
            self.factory.params().samples,
        );
    }
}
