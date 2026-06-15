use std::ops::{Deref, DerefMut};

use crate::synth_engine::{
    Sample, SpectralBuffer,
    buffer::{VoicesLayout, VoicesLayoutArray, add_to_buffer, new_voices_layout},
    routing::{
        InputSlot,
        outputs::{SamplesOutput, SpectralOutput},
    },
};

pub(super) struct ArenaSlot<T: Default + Send> {
    pub(super) slot: Option<VoicesLayout<T>>,
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
    pub(super) samples: Vec<ArenaSlot<SamplesOutput>>,
    pub(super) spectral: Vec<ArenaSlot<SpectralOutput>>,
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

    pub(super) fn get_buff(
        &self,
        slot: Option<usize>,
        channel_idx: usize,
        voice_idx: usize,
    ) -> Option<&[Sample]> {
        slot.map(|slot| self.samples[slot][channel_idx][voice_idx].buffer())
    }

    pub(super) fn add_buff_to(
        &self,
        slots: &[InputSlot],
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

    pub(super) fn get_scalar(
        &self,
        slots: &[InputSlot],
        channel_idx: usize,
        voice_idx: usize,
        triggered: bool,
    ) -> Option<Sample> {
        if slots.is_empty() {
            return None;
        }

        let mut result: Sample = 0.0;

        for slot in slots {
            let mut value = self.samples[slot.src_slot][channel_idx][voice_idx].scalar(triggered)
                * slot.amount[channel_idx];

            if let Some(modulated_slot) = slot.modulation_slot {
                value *= self.samples[modulated_slot][channel_idx][voice_idx].scalar(triggered);
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
        triggered: bool,
    ) -> Option<&SpectralBuffer> {
        slot.map(|slot| self.spectral[slot][channel_idx][voice_idx].get(triggered))
    }
}
