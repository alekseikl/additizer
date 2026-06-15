use crate::synth_engine::{
    Buffer, ModuleId, ProcessParams, Sample, SpectralBuffer,
    buffer::{VoicesLayout, ZEROES_BUFFER, ZEROES_SPECTRAL_BUFFER},
    routing::{InputSlots, ProcessContext, SamplesOutput, SpectralOutput},
    smooth::SmoothedSample,
};

pub trait RouterDataType {}

pub struct AudioRouterType {
    pub(super) samples_slot: usize,
}

impl RouterDataType for AudioRouterType {}

pub struct ControlRouterType {
    pub(super) samples_slot: usize,
}

impl RouterDataType for ControlRouterType {}

pub struct SpectralRouterType {
    pub(super) spectral_slot: usize,
}

impl RouterDataType for SpectralRouterType {}

pub struct OutputRouterType;

impl RouterDataType for OutputRouterType {}

pub struct RouterFactory<'f, 'c, S: RouterDataType> {
    pub(super) ctx: &'f mut ProcessContext<'c>,
    pub(super) module_id: ModuleId,
    pub(super) data_type: S,
}

impl<'f, 'c, S: RouterDataType> RouterFactory<'f, 'c, S> {
    pub fn params(&self) -> &ProcessParams<'_> {
        &self.ctx.params
    }

    pub fn for_voice<'voice>(
        &'voice mut self,
        channel_idx: usize,
        voice_idx: usize,
        seq_idx: usize,
    ) -> VoiceRouter<'voice, 'f, 'c, S>
    where
        'f: 'voice,
    {
        VoiceRouter {
            factory: self,
            channel_idx,
            voice_idx,
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
}

pub struct VoiceRouter<'v, 'f, 'c, S: RouterDataType> {
    factory: &'v mut RouterFactory<'f, 'c, S>,
    channel_idx: usize,
    voice_idx: usize,
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
        self.voice_idx
    }

    fn buff_impl(&mut self, slot: Option<usize>) -> &[Sample] {
        self.factory
            .ctx
            .outputs_arena
            .get_buff(slot, self.channel_idx, self.voice_idx)
            .unwrap_or(&ZEROES_BUFFER)
    }

    fn scalar_param_impl(&mut self, input: &InputSlots, param: Sample, triggered: bool) -> Sample {
        if let Some(value) = self.factory.ctx.outputs_arena.get_scalar(
            &input.slots,
            self.channel_idx,
            self.voice_idx,
            triggered,
        ) {
            let value = value + param;

            if self.factory.ctx.params.needs_update_ui && self.seq_idx == 0 {
                self.factory.ctx.audio_end.update_modulated_input(
                    self.factory.module_id,
                    input.input_type,
                    self.channel_idx as u8,
                    value,
                );
            }

            value
        } else {
            param
        }
    }

    fn spectral_impl(&self, slot: Option<usize>, triggered: bool) -> &SpectralBuffer {
        self.factory
            .ctx
            .outputs_arena
            .get_spectral(slot, self.channel_idx, self.voice_idx, triggered)
            .unwrap_or(&ZEROES_SPECTRAL_BUFFER)
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
            self.voice_idx,
            0,
            buff,
        ) {
            self.factory.ctx.audio_end.update_modulated_input(
                self.factory.module_id,
                input.input_type,
                self.channel_idx as u8,
                buff[0],
            );
        }
    }

    pub fn scalar_param(&mut self, input: &InputSlots, param: Sample, triggered: bool) -> Sample {
        self.scalar_param_impl(input, param, triggered)
    }

    pub fn spectral(&self, slot: Option<usize>, triggered: bool) -> &SpectralBuffer {
        self.spectral_impl(slot, triggered)
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
        let buff = &mut buff[skip..params.samples + 1];

        if param.check_needs_smoothing(&params.smooth_params) {
            param.smoothed_buff(buff, &params.smooth_params);
        } else {
            buff.fill(param.get());
        }

        if self.factory.ctx.outputs_arena.add_buff_to(
            &input.slots,
            self.channel_idx,
            self.voice_idx,
            skip,
            buff,
        ) {
            self.factory.ctx.audio_end.update_modulated_input(
                self.factory.module_id,
                input.input_type,
                self.channel_idx as u8,
                buff[skip],
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

    pub fn spectral(&self, slot: Option<usize>, triggered: bool) -> &SpectralBuffer {
        self.spectral_impl(slot, triggered)
    }
}

impl<'v, 'f, 'c> VoiceRouter<'v, 'f, 'c, OutputRouterType> {
    pub fn buff(&mut self, slot: Option<usize>) -> &[Sample] {
        self.buff_impl(slot)
    }
}
