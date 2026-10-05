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
    svf::{SvfFilter, SvfState, SvfType},
};

use crate::{
    synth_engine::{C4_PITCH, MAX_CUTOFF, MIN_CUTOFF, db_to_gain_fast},
    synth_engine::{
        Sample, SmoothedSampleParams, StereoSample,
        buffer::{Buffer, VoicesLayout, new_voices_layout, zero_buffer},
        routing::{
            AudioRouterType, DataType, Input, InputMeta, InputSlots, ModuleId, NUM_CHANNELS,
            ProcessContext, RouterFactory, SamplesOutput, SpectralInputSlot, VoiceEvent,
            VoiceTarget,
        },
        smooth::SmoothedSample,
        synth_module::SynthModule,
    },
};

struct Params {
    filter_type: SvfType,
    keytrack: Sample,
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
    cutoff: InputSlots,
    resonance: InputSlots,
    drive: InputSlots,
}

impl Default for Inputs {
    fn default() -> Self {
        Self {
            audio: None,
            pitch: None,
            cutoff: InputSlots::new(Input::Cutoff),
            resonance: InputSlots::new(Input::Resonance),
            drive: InputSlots::new(Input::Drive),
        }
    }
}

impl Inputs {
    fn from_slots(inputs: &[InputSlots], _spectral_inputs: &[SpectralInputSlot]) -> Self {
        let mut result = Self::default();

        for input in inputs {
            match input.input_type {
                Input::Audio => result.audio = input.slots.first().map(|s| s.src_slot),
                Input::Pitch => result.pitch = input.slots.first().map(|s| s.src_slot),
                Input::Cutoff => result.cutoff = input.clone(),
                Input::Resonance => result.resonance = input.clone(),
                Input::Drive => result.drive = input.clone(),
                _ => (),
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
    cutoff: Buffer,
    resonance: Buffer,
    pre_k: Buffer,
    drive: Buffer,
}

pub struct Svf<L: SvfLinks = stub::Links> {
    id: ModuleId,
    params: Params,
    channel_params: [ChannelParams; NUM_CHANNELS],
    buffers: Buffers,
    states: VoicesLayout<SvfState>,
    audio_end: L::AudioEnd,
    ui_end: Option<L::UiEnd>,
    inputs: Inputs,
    output_slot: usize,
}

impl<L: SvfLinks> Svf<L> {
    pub fn take_ui_end(&mut self) -> Option<L::UiEnd> {
        self.ui_end.take()
    }

    pub fn new(id: ModuleId) -> Self {
        Self::from_config(&SvfConfig {
            id,
            ..SvfConfig::default()
        })
    }

    pub fn from_config(config: &SvfConfig) -> Self {
        let (audio_end, ui_end) = L::create_link_pair();

        Self {
            id: config.id,
            params: Params::from_config(config),
            channel_params: array::from_fn(|channel_idx| {
                ChannelParams::from_config(config, channel_idx)
            }),
            buffers: Buffers {
                cutoff: zero_buffer(),
                resonance: zero_buffer(),
                pre_k: zero_buffer(),
                drive: zero_buffer(),
            },
            states: new_voices_layout(),
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
    set_mono_param!(set_keytrack, keytrack, Sample, keytrack.clamp(0.0, 1.0));

    set_smoothed_param!(set_cutoff, cutoff, cutoff.clamp(MIN_CUTOFF, MAX_CUTOFF));
    set_smoothed_param!(
        set_resonance,
        resonance,
        resonance.clamp(MIN_RESONANCE, MAX_RESONANCE)
    );
    set_smoothed_param!(set_drive, drive, drive.clamp(MIN_DRIVE, MAX_DRIVE));

    fn reset_voice(&mut self, voice_idx: usize) {
        for state in self.states.channels_at_mut(voice_idx) {
            *state = SvfState::new(self.params.filter_type);
        }
    }

    fn process_voice(
        &mut self,
        target: &VoiceTarget,
        outputs: &mut VoicesLayout<SamplesOutput>,
        rf: &mut RouterFactory<AudioRouterType, L::EngineEnd>,
    ) {
        let (mut router, mut voice_output) = rf.for_voice(target, outputs);
        let inputs = &self.inputs;
        let params = &self.params;
        let channel = &self.channel_params[target.channel_idx];
        let state = self.states.at_mut(target.channel_idx, target.voice_idx);
        let sample_rate = router.sample_rate();
        let filter_type = params.filter_type;

        state.set_type(filter_type);

        router.param(&inputs.cutoff, &channel.cutoff, &mut self.buffers.cutoff);

        let samples = router.samples();

        if let Some(resonance) = router.param_stationary(&inputs.resonance, &channel.resonance) {
            let q = q_from_resonance(resonance);

            self.buffers.resonance[..samples].fill(q.resonant.recip());
            self.buffers.pre_k[..samples].fill(q.pre_stage.recip());
        } else {
            router.param(
                &inputs.resonance,
                &channel.resonance,
                &mut self.buffers.resonance,
            );

            for (resonance, pre_k) in self
                .buffers
                .resonance
                .iter_mut()
                .zip(&mut self.buffers.pre_k)
                .take(samples)
            {
                let q = q_from_resonance(*resonance);

                *resonance = q.resonant.recip();
                *pre_k = q.pre_stage.recip();
            }
        }

        if let Some(drive) = router.param_stationary(&inputs.drive, &channel.drive) {
            let gain = db_to_gain_fast(drive.clamp(MIN_DRIVE, MAX_DRIVE));

            self.buffers.drive[..samples].fill(gain);
        } else {
            router.param(&inputs.drive, &channel.drive, &mut self.buffers.drive);

            for drive in &mut self.buffers.drive[..samples] {
                *drive = db_to_gain_fast(drive.clamp(MIN_DRIVE, MAX_DRIVE));
            }
        }

        let note_pitch = target.note_pitch();
        let pitch = inputs.pitch.is_some().then(|| router.direct(inputs.pitch));
        let input = router.direct(inputs.audio);
        let output = voice_output.output();

        if router.need_update_ui_mono() {
            let pitch = pitch.and_then(|p| p.first().copied()).unwrap_or(note_pitch);

            self.audio_end.update_pitch(pitch);
        }

        let keytrack = params.keytrack;

        if keytrack > 1e-5 {
            if let Some(pitch) = pitch {
                for (cutoff, pitch) in self.buffers.cutoff.iter_mut().zip(pitch) {
                    *cutoff += keytrack * (pitch - C4_PITCH);
                }
            } else {
                let offset = keytrack * (note_pitch - C4_PITCH);

                for cutoff in self.buffers.cutoff.iter_mut().take(samples) {
                    *cutoff += offset;
                }
            }
        }

        state.process(
            sample_rate,
            input,
            &self.buffers.cutoff,
            &self.buffers.resonance,
            &self.buffers.pre_k,
            &self.buffers.drive,
            output,
        );
    }
    pub(crate) fn process(&mut self, ctx: &mut ProcessContext<L::EngineEnd>) {
        ctx.audio(self.id, self.output_slot)
            .for_voices(|rf, target, outputs| {
                self.process_voice(target, outputs, rf);
            })
            .for_channels(|rf, channel_idx| {
                self.channel_params[channel_idx]
                    .advance_smoothers(&rf.params().smooth_params, rf.params().samples);
            });
    }
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

    fn set_input_slots(&mut self, inputs: &[InputSlots], spectral_inputs: &[SpectralInputSlot]) {
        self.inputs = Inputs::from_slots(inputs, spectral_inputs);
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
