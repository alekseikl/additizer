use std::ops::{Deref, DerefMut};

use crate::synth_engine::{
    Sample, SpectralBuffer, SynthModule,
    buffer::{VoicesLayout, VoicesLayoutArray, add_to_buffer, new_voices_layout},
    module_handle::ModuleHandle,
    routing::{
        DataType, InputSlot,
        outputs::{SamplesOutput, SpectralOutput},
    },
};

pub(super) struct ArenaSlot<T: Default + Send> {
    pub(super) slot: Option<VoicesLayout<T>>,
    pub(super) is_control: bool,
}

impl<T: Default + Send> Default for ArenaSlot<T> {
    fn default() -> Self {
        Self {
            slot: Some(new_voices_layout()),
            is_control: false,
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
    pub(super) samples: Vec<ArenaSlot<SamplesOutput>>,
    pub(super) spectral: Vec<ArenaSlot<SpectralOutput>>,
    free_samples_slots: Vec<usize>,
    free_spectral_slots: Vec<usize>,
}

impl OutputsArena {
    pub fn new() -> Self {
        Self {
            samples: Vec::new(),
            spectral: Vec::new(),
            free_samples_slots: Vec::new(),
            free_spectral_slots: Vec::new(),
        }
    }

    fn allocate_impl<T: Default + Send>(
        slots: &mut Vec<ArenaSlot<T>>,
        free_slots: &mut Vec<usize>,
    ) -> usize {
        if let Some(slot_idx) = free_slots.pop() {
            slots[slot_idx].slot = Some(new_voices_layout());
            slot_idx
        } else {
            slots.push(ArenaSlot::default());
            slots.len() - 1
        }
    }

    fn allocate_samples_slot(&mut self, is_control: bool) -> usize {
        let slot_idx = Self::allocate_impl(&mut self.samples, &mut self.free_samples_slots);
        self.samples[slot_idx].is_control = is_control;
        slot_idx
    }

    fn allocate_spectral_slot(&mut self) -> usize {
        Self::allocate_impl(&mut self.spectral, &mut self.free_spectral_slots)
    }

    fn free_impl<T: Default + Send>(
        slots: &mut [ArenaSlot<T>],
        free_slots: &mut Vec<usize>,
        slot: usize,
    ) {
        assert!(slot < slots.len());

        slots[slot].slot = None;
        free_slots.push(slot);
    }

    fn free_samples_slot(&mut self, slot: usize) {
        Self::free_impl(&mut self.samples, &mut self.free_samples_slots, slot);
    }

    fn free_spectral_slot(&mut self, slot: usize) {
        Self::free_impl(&mut self.spectral, &mut self.free_spectral_slots, slot);
    }

    pub fn allocate_slot(&mut self, module: &mut ModuleHandle) {
        match module.output_type() {
            DataType::Audio => module.set_output_slot(self.allocate_samples_slot(false)),
            DataType::Control => module.set_output_slot(self.allocate_samples_slot(true)),
            DataType::Spectral => module.set_output_slot(self.allocate_spectral_slot()),
        }
    }

    pub fn free_slot(&mut self, module: &ModuleHandle) {
        match module.output_type() {
            DataType::Audio | DataType::Control => self.free_samples_slot(module.output_slot()),
            DataType::Spectral => self.free_spectral_slot(module.output_slot()),
        }
    }

    pub(super) fn get_buff(
        &self,
        slot: Option<usize>,
        channel_idx: usize,
        voice_idx: usize,
    ) -> Option<&[Sample]> {
        slot.map(|slot| self.samples[slot][channel_idx][voice_idx].buffer())
    }

    fn assert_control_rate(&self, slot: usize) {
        assert!(
            self.samples[slot].is_control,
            "control-rate read of audio-rate output slot {slot}"
        );
    }

    pub(super) fn add_buff_to(
        &self,
        slots: &[InputSlot],
        is_control: bool,
        channel_idx: usize,
        voice_idx: usize,
        offset: usize,
        result: &mut [Sample], // Number of samples is controlled but result slice length
    ) -> bool {
        if slots.is_empty() {
            return false;
        }

        for slot in slots {
            if is_control {
                self.assert_control_rate(slot.src_slot);
                if let Some(modulation_slot) = slot.modulation_slot {
                    self.assert_control_rate(modulation_slot);
                }
            }

            let amount = slot.amount[channel_idx];
            let input = self.samples[slot.src_slot][channel_idx][voice_idx]
                .buffer()
                .iter()
                .skip(offset)
                .map(|sample| sample * amount);

            if let Some(modulation_slot) = slot.modulation_slot {
                let input_mod = self.samples[modulation_slot][channel_idx][voice_idx]
                    .buffer()
                    .iter()
                    .skip(offset);

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

    fn control_scalar(
        &self,
        slot: usize,
        channel_idx: usize,
        voice_idx: usize,
        this_frame: Option<usize>,
    ) -> Sample {
        self.assert_control_rate(slot);
        self.samples[slot][channel_idx][voice_idx].scalar(this_frame)
    }

    pub(super) fn get_scalar(
        &self,
        slots: &[InputSlot],
        channel_idx: usize,
        voice_idx: usize,
        this_frame: Option<usize>,
    ) -> Option<Sample> {
        if slots.is_empty() {
            return None;
        }

        let mut result: Sample = 0.0;

        for slot in slots {
            let mut value = self.control_scalar(slot.src_slot, channel_idx, voice_idx, this_frame)
                * slot.amount[channel_idx];

            if let Some(modulated_slot) = slot.modulation_slot {
                value *= self.control_scalar(modulated_slot, channel_idx, voice_idx, this_frame);
            }

            result += value;
        }

        Some(result)
    }

    pub(super) fn get_spectral(
        &self,
        slot: Option<usize>,
        channel_idx: usize,
        voice_idx: usize,
    ) -> Option<&SpectralBuffer> {
        slot.map(|slot| self.spectral[slot][channel_idx][voice_idx].get())
    }
}

#[cfg(test)]
mod tests;
