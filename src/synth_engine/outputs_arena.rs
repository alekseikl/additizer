use std::ops::{Deref, DerefMut};

use crate::synth_engine::{
    Buffer, Input, ModuleId, ProcessParams, Sample, SpectralBuffer, StereoSample,
    buffer::{
        VoicesLayout, VoicesLayoutArray, ZEROES_BUFFER, ZEROES_SPECTRAL_BUFFER, add_to_buffer,
        new_voices_layout,
    },
    smooth::SmoothedSample,
    types::{SamplesOutput, SpectralOutput},
    ui_bridge::AudioEnd,
};

#[derive(Clone)]
pub struct SamplesInputSrc {
    pub src_slot: usize,
    pub modulation_slot: Option<usize>,
    pub amount: StereoSample,
}

#[derive(Clone)]
pub struct InputSlots {
    pub input_type: Input,
    pub slots: Vec<SamplesInputSrc>,
}

impl InputSlots {
    pub fn empty(input_type: Input) -> Self {
        Self {
            input_type,
            slots: Vec::new(),
        }
    }
}

pub struct SpectralInputSlot {
    pub input_type: Input,
    pub slot: usize,
}

struct ArenaSlot<T: Default + Send> {
    slot: Option<VoicesLayout<T>>,
}

impl<T: Default + Send> Default for ArenaSlot<T> {
    fn default() -> Self {
        Self {
            slot: Some(new_voices_layout()),
        }
    }
}

impl<T: Default + Send> Deref for ArenaSlot<T> {
    type Target = VoicesLayoutArray<T>;

    fn deref(&self) -> &Self::Target {
        self.slot
            .as_deref()
            .expect("buffer slot should be in place")
    }
}

impl<T: Default + Send> DerefMut for ArenaSlot<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.slot
            .as_deref_mut()
            .expect("buffer slot should be in place")
    }
}

pub struct OutputsArena {
    samples: Vec<ArenaSlot<SamplesOutput>>,
    spectral: Vec<ArenaSlot<SpectralOutput>>,
}

impl OutputsArena {
    pub fn new() -> Self {
        Self {
            samples: Vec::new(),
            spectral: Vec::new(),
        }
    }

    pub fn set_num_slots(&mut self, samples_slots: usize, spectral_slots: usize) {
        self.samples.resize_with(samples_slots, ArenaSlot::default);
        self.spectral
            .resize_with(spectral_slots, ArenaSlot::default);
    }

    pub fn get_buff(
        &self,
        slot: Option<usize>,
        channel_idx: usize,
        voice_idx: usize,
    ) -> Option<&[Sample]> {
        slot.map(|slot| self.samples[slot][channel_idx][voice_idx].buffer())
    }

    pub fn add_buff_to(
        &self,
        slots: &[SamplesInputSrc],
        channel_idx: usize,
        voice_idx: usize,
        skip: usize,
        result: &mut [Sample], // Number of samples is controlled but result slice length
    ) -> bool {
        if slots.is_empty() {
            return false;
        }

        for slot in slots {
            let amount = slot.amount[channel_idx];
            let input = self.samples[slot.src_slot][channel_idx][voice_idx]
                .buffer()
                .iter()
                .skip(skip)
                .map(|sample| sample * amount);

            if let Some(modulation_slot) = slot.modulation_slot {
                let input_mod = self.samples[modulation_slot][channel_idx][voice_idx]
                    .buffer()
                    .iter()
                    .skip(skip);

                add_to_buffer(
                    result,
                    input
                        .zip(input_mod)
                        .map(|(input, input_mod)| input * input_mod),
                );
            } else {
                add_to_buffer(result, input);
            }
        }

        true
    }

    pub fn get_scalar(
        &self,
        slots: &[SamplesInputSrc],
        channel_idx: usize,
        voice_idx: usize,
        next_frame: bool,
    ) -> Option<Sample> {
        if slots.is_empty() {
            return None;
        }

        let mut result: Sample = 0.0;

        for slot in slots {
            let mut value = self.samples[slot.src_slot][channel_idx][voice_idx].scalar(next_frame)
                * slot.amount[channel_idx];

            if let Some(modulated_slot) = slot.modulation_slot {
                value *= self.samples[modulated_slot][channel_idx][voice_idx].scalar(next_frame);
            }

            result += value;
        }

        Some(result)
    }

    pub fn get_spectral(
        &self,
        slot: Option<usize>,
        channel_idx: usize,
        voice_idx: usize,
        next_frame: bool,
    ) -> Option<&SpectralBuffer> {
        slot.map(|slot| self.spectral[slot][channel_idx][voice_idx].get(next_frame))
    }
}

pub struct ProcessContext<'c> {
    pub outputs_arena: &'c mut OutputsArena,
    pub audio_end: &'c mut AudioEnd,
    pub params: ProcessParams<'c>,
}

impl<'c> ProcessContext<'c> {
    pub fn for_audio<'f>(
        &'f mut self,
        module_id: ModuleId,
        output_slot: usize,
        f: impl FnOnce(&mut RouterFactory<'f, 'c, AudioRouterType>, &mut VoicesLayout<SamplesOutput>),
    ) where
        'c: 'f,
    {
        RouterFactory {
            ctx: self,
            module_id,
            output_slots: AudioRouterType {
                samples_slot: output_slot,
            },
        }
        .with_output_slot(f);
    }

    pub fn for_control<'f>(
        &'f mut self,
        module_id: ModuleId,
        output_slot: usize,
        f: impl FnOnce(&mut RouterFactory<'f, 'c, ControlRouterType>, &mut VoicesLayout<SamplesOutput>),
    ) where
        'c: 'f,
    {
        RouterFactory {
            ctx: self,
            module_id,
            output_slots: ControlRouterType {
                samples_slot: output_slot,
            },
        }
        .with_output_slot(f);
    }

    pub fn for_spectral<'f>(
        &'f mut self,
        module_id: ModuleId,
        output_slot: usize,
        f: impl FnOnce(
            &mut RouterFactory<'f, 'c, SpectralRouterType>,
            &mut VoicesLayout<SpectralOutput>,
        ),
    ) where
        'c: 'f,
    {
        RouterFactory {
            ctx: self,
            module_id,
            output_slots: SpectralRouterType {
                spectral_slot: output_slot,
            },
        }
        .with_output_slot(f);
    }
}

pub trait RouterDataType {}

pub struct AudioRouterType {
    samples_slot: usize,
}

impl RouterDataType for AudioRouterType {}

pub struct ControlRouterType {
    samples_slot: usize,
}

impl RouterDataType for ControlRouterType {}

pub struct SpectralRouterType {
    spectral_slot: usize,
}

impl RouterDataType for SpectralRouterType {}

pub struct RouterFactory<'f, 'c, S: RouterDataType> {
    ctx: &'f mut ProcessContext<'c>,
    module_id: ModuleId,
    output_slots: S,
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
        let mut slot = self.ctx.outputs_arena.samples[self.output_slots.samples_slot]
            .slot
            .take()
            .expect("slot should be in place");

        f(self, &mut slot);

        self.ctx.outputs_arena.samples[self.output_slots.samples_slot]
            .slot
            .replace(slot);
    }
}

impl<'f, 'c> RouterFactory<'f, 'c, ControlRouterType> {
    pub fn with_output_slot(
        &mut self,
        f: impl FnOnce(&mut Self, &mut VoicesLayout<SamplesOutput>),
    ) {
        let mut slot = self.ctx.outputs_arena.samples[self.output_slots.samples_slot]
            .slot
            .take()
            .expect("slot should be in place");

        f(self, &mut slot);

        self.ctx.outputs_arena.samples[self.output_slots.samples_slot]
            .slot
            .replace(slot);
    }
}

impl<'f, 'c> RouterFactory<'f, 'c, SpectralRouterType> {
    pub fn with_output_slot(
        &mut self,
        f: impl FnOnce(&mut Self, &mut VoicesLayout<SpectralOutput>),
    ) {
        let mut slot = self.ctx.outputs_arena.spectral[self.output_slots.spectral_slot]
            .slot
            .take()
            .expect("slot should be in place");

        f(self, &mut slot);

        self.ctx.outputs_arena.spectral[self.output_slots.spectral_slot]
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

    fn scalar_param_impl(&mut self, input: &InputSlots, param: Sample, next_frame: bool) -> Sample {
        if let Some(value) = self.factory.ctx.outputs_arena.get_scalar(
            &input.slots,
            self.channel_idx,
            self.voice_idx,
            next_frame,
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

    pub fn spectral_impl(&self, slot: Option<usize>, next_frame: bool) -> &SpectralBuffer {
        self.factory
            .ctx
            .outputs_arena
            .get_spectral(slot, self.channel_idx, self.voice_idx, next_frame)
            .unwrap_or(&ZEROES_SPECTRAL_BUFFER)
    }
}

impl<'v, 'f, 'c> VoiceRouter<'v, 'f, 'c, AudioRouterType> {
    pub fn buff(&mut self, slot: Option<usize>) -> &[Sample] {
        self.factory
            .ctx
            .outputs_arena
            .get_buff(slot, self.channel_idx, self.voice_idx)
            .unwrap_or(&ZEROES_BUFFER)
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

    pub fn scalar_param(&mut self, input: &InputSlots, param: Sample, next_frame: bool) -> Sample {
        self.scalar_param_impl(input, param, next_frame)
    }

    pub fn spectral(&self, slot: Option<usize>, next_frame: bool) -> &SpectralBuffer {
        self.spectral_impl(slot, next_frame)
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
        let skip = usize::from(triggered);
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

    pub fn scalar_param(&mut self, input: &InputSlots, param: Sample, next_frame: bool) -> Sample {
        self.scalar_param_impl(input, param, next_frame)
    }
}

impl<'v, 'f, 'c> VoiceRouter<'v, 'f, 'c, SpectralRouterType> {
    pub fn scalar_param(&mut self, input: &InputSlots, param: Sample, next_frame: bool) -> Sample {
        self.scalar_param_impl(input, param, next_frame)
    }

    pub fn spectral(&self, slot: Option<usize>, next_frame: bool) -> &SpectralBuffer {
        self.spectral_impl(slot, next_frame)
    }
}
