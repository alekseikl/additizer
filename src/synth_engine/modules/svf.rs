use std::array;

use itertools::izip;

mod config;
mod link;
mod ui_bridge;

#[cfg(test)]
mod tests;

pub use config::SvfConfig;
use link::{AudioEnd, UiEnd, UiEvent, create_link_pair};
pub use ui_bridge::SvfUiBridge;

use crate::{
    synth_engine::{
        Sample, SmoothedSampleParams, StereoSample,
        buffer::{Buffer, VoicesLayout, zero_buffer},
        filters::svf::{SvfCoeffs, SvfState, SvfType},
        modules::spectral_filter::{
            MAX_DRIVE, MAX_RESONANCE, MIN_DRIVE, MIN_RESONANCE, q_from_resonance,
        },
        routing::{
            AudioRouterType, DataType, Input, InputMeta, InputSlots, MAX_VOICES, ModuleId,
            NUM_CHANNELS, ProcessContext, RouterFactory, SamplesOutput, SpectralInputSlot,
            VoiceEvent, VoiceTarget,
        },
        smooth::SmoothedSample,
        synth_module::SynthModule,
    },
    utils::{C4_PITCH, MAX_CUTOFF, MIN_CUTOFF, db_to_gain_fast, pitch_to_freq},
};

/// Lowest cutoff frequency the filter is ever tuned to, Hz.
pub const MIN_CUTOFF_FREQ: Sample = 5.0;
/// Highest cutoff as a fraction of the sample rate (keeps `tan` well away from Nyquist).
pub const MAX_CUTOFF_RATIO: Sample = 0.45;

/// Absolute cutoff in octave units (same space as pitch) for the given settings.
pub fn cutoff_pitch(cutoff: Sample, keytrack: Sample, pitch: Sample) -> Sample {
    C4_PITCH + cutoff.clamp(MIN_CUTOFF, MAX_CUTOFF) + keytrack * (pitch - C4_PITCH)
}

/// Cutoff frequency in Hz, clamped to the range the filter can be tuned to.
pub fn cutoff_freq(cutoff: Sample, keytrack: Sample, pitch: Sample, sample_rate: Sample) -> Sample {
    pitch_to_freq(cutoff_pitch(cutoff, keytrack, pitch))
        .clamp(MIN_CUTOFF_FREQ, sample_rate * MAX_CUTOFF_RATIO)
}

/// Drive: dB of gain into a `tanh` soft clipper ahead of the filter.
#[inline(always)]
fn saturate(input: Sample, drive: Sample) -> Sample {
    (input * db_to_gain_fast(drive.clamp(MIN_DRIVE, MAX_DRIVE))).tanh()
}

struct Params {
    filter_type: SvfType,
    /// 0 = absolute cutoff (relative to C4), 1 = full key tracking.
    keytrack: Sample,
}

impl Params {
    fn from_config(c: &SvfConfig) -> Self {
        Self {
            filter_type: c.filter_type,
            keytrack: c.keytrack.clamp(0.0, 1.0),
        }
    }

    fn coeffs(
        &self,
        cutoff: Sample,
        resonance: Sample,
        pitch: Sample,
        sample_rate: Sample,
    ) -> SvfCoeffs {
        SvfCoeffs::new(
            cutoff_freq(cutoff, self.keytrack, pitch, sample_rate),
            q_from_resonance(resonance.clamp(MIN_RESONANCE, MAX_RESONANCE)),
            sample_rate,
        )
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
            resonance: c.resonance[channel_idx].into(),
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
    cutoff_mod: Buffer,
    resonance_mod: Buffer,
    drive_mod: Buffer,
}

pub struct Svf {
    id: ModuleId,
    params: Params,
    channel_params: [ChannelParams; NUM_CHANNELS],
    buffers: Buffers,
    states: [[SvfState; MAX_VOICES]; NUM_CHANNELS],
    audio_end: AudioEnd,
    ui_end: Option<UiEnd>,
    inputs: Inputs,
    output_slot: usize,
}

impl Svf {
    pub fn new(id: ModuleId) -> Self {
        Self::from_config(&SvfConfig {
            id,
            ..SvfConfig::default()
        })
    }

    pub fn from_config(config: &SvfConfig) -> Self {
        let (audio_end, ui_end) = create_link_pair();

        Self {
            id: config.id,
            params: Params::from_config(config),
            channel_params: array::from_fn(|channel_idx| {
                ChannelParams::from_config(config, channel_idx)
            }),
            buffers: Buffers {
                cutoff_mod: zero_buffer(),
                resonance_mod: zero_buffer(),
                drive_mod: zero_buffer(),
            },
            states: [[SvfState::default(); MAX_VOICES]; NUM_CHANNELS],
            audio_end,
            ui_end: Some(ui_end),
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
        for channel in self.states.iter_mut() {
            channel[voice_idx].reset();
        }
    }

    fn process_voice(
        &mut self,
        target: &VoiceTarget,
        outputs: &mut VoicesLayout<SamplesOutput>,
        rf: &mut RouterFactory<AudioRouterType>,
    ) {
        let smooth_params = rf.params().smooth_params;
        let (mut router, mut voice_output) = rf.for_voice(target, outputs);
        let inputs = &self.inputs;
        let params = &self.params;
        let channel = &self.channel_params[target.channel_idx];
        let state = &mut self.states[target.channel_idx][target.voice_idx];
        let sample_rate = router.sample_rate();

        // A NaN/inf that slipped in would otherwise stick forever.
        if !state.is_finite() {
            state.reset();
        }

        router.param(
            &inputs.cutoff,
            &channel.cutoff,
            &mut self.buffers.cutoff_mod,
        );
        router.param(
            &inputs.resonance,
            &channel.resonance,
            &mut self.buffers.resonance_mod,
        );
        router.param(&inputs.drive, &channel.drive, &mut self.buffers.drive_mod);

        let pitch_tracked = inputs.pitch.is_some() && params.keytrack != 0.0;
        let coeffs_static = !pitch_tracked
            && inputs.cutoff.is_empty()
            && inputs.resonance.is_empty()
            && !channel.cutoff.check_needs_smoothing(&smooth_params)
            && !channel.resonance.check_needs_smoothing(&smooth_params);

        let note_pitch = target.note_pitch();
        let pitch = inputs.pitch.is_some().then(|| router.direct(inputs.pitch));
        let pitch_at = |idx: usize| pitch.map_or(note_pitch, |p| p[idx]);
        let input = router.direct(inputs.audio);
        let output = voice_output.output();
        let cutoff_mod = &self.buffers.cutoff_mod[..input.len()];
        let resonance_mod = &self.buffers.resonance_mod[..input.len()];
        let drive_mod = &self.buffers.drive_mod[..input.len()];
        let filter_type = params.filter_type;

        if router.need_update_ui_mono() {
            self.audio_end.update_pitch(pitch_at(0));
        }

        if input.is_empty() {
            return;
        }

        if coeffs_static {
            let coeffs = params.coeffs(cutoff_mod[0], resonance_mod[0], pitch_at(0), sample_rate);

            for (out, &x, &drive) in izip!(output.iter_mut(), input, drive_mod) {
                *out = state.tick(filter_type, &coeffs, saturate(x, drive));
            }
        } else {
            for (idx, (out, &x, &drive, &cutoff, &resonance)) in izip!(
                output.iter_mut(),
                input,
                drive_mod,
                cutoff_mod,
                resonance_mod
            )
            .enumerate()
            {
                let coeffs = params.coeffs(cutoff, resonance, pitch_at(idx), sample_rate);

                *out = state.tick(filter_type, &coeffs, saturate(x, drive));
            }
        }
    }
}

impl SynthModule for Svf {
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

    fn process(&mut self, ctx: &mut ProcessContext) {
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
