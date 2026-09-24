use std::{array, f32};

use rand::RngExt;
use rand_pcg::Pcg32;

use crate::{
    synth_engine::{
        ComplexSample, MAX_BANDWIDTH, Sample, StereoSample,
        buffer::{DC_OFFSET, SPECTRAL_BUFFER_SIZE, VoicesLayout},
        routing::{
            DataType, Input, InputMeta, InputSlots, LEFT_CHANNEL, MAX_VOICES, ModuleId,
            NUM_CHANNELS, ProcessContext, RouterFactory, SpectralInputSlot, SpectralOutput,
            SpectralRouterType, VoiceEvent, VoiceTarget,
        },
        synth_module::SynthModule,
        voices_handler::BAND_LIMIT_FREQUENCY,
    },
    utils::{C4_PITCH, MAX_LEVEL_DB, MIN_LEVEL_DB, db_to_gain, pitch_to_freq},
};

use crate::synth_engine::spectral_filter::{MAX_CUTOFF, MIN_CUTOFF};

mod config;
mod link;
mod ui_bridge;

#[cfg(test)]
mod tests;

pub use config::{NoiseColor, SpectralNoiseConfig};
pub use ui_bridge::SpectralNoiseUiBridge;

use link::{AudioEnd, UiEnd, UiEvent, create_link_pair};

/// Matches the spectral filter's Q-limit slope: `1` is this many times steeper than `0`.
const MAX_AMOUNT_LIMIT_POWER: Sample = 20.0;

#[derive(Clone, Copy)]
struct PhaseReset {
    steal_from: Option<usize>,
}

struct Inputs {
    pitch: Option<usize>,
    amount: InputSlots,
    level: InputSlots,
}

impl Default for Inputs {
    fn default() -> Self {
        Self {
            pitch: None,
            amount: InputSlots::new(Input::Amount),
            level: InputSlots::new(Input::Level),
        }
    }
}

impl Inputs {
    fn from_slots(inputs: &[InputSlots]) -> Self {
        let mut result = Self::default();

        for input in inputs {
            match input.input_type {
                Input::Pitch => result.pitch = input.slots.first().map(|s| s.src_slot),
                Input::Amount => result.amount = input.clone(),
                Input::Level => result.level = input.clone(),
                _ => (),
            }
        }

        result
    }

    fn update_amount(&mut self, input_type: Input, src_slot: usize, amount: StereoSample) {
        match input_type {
            Input::Amount => self.amount.update_amount(src_slot, amount),
            Input::Level => self.level.update_amount(src_slot, amount),
            _ => (),
        }
    }
}

struct ChannelParams {
    amount: Sample,
    level: Sample,
}

impl ChannelParams {
    fn from_config(config: &SpectralNoiseConfig, channel_idx: usize) -> Self {
        Self {
            amount: SpectralNoise::clamp_amount(config.amount[channel_idx]),
            level: SpectralNoise::clamp_level(config.level[channel_idx]),
        }
    }
}

pub struct SpectralNoise {
    id: ModuleId,
    audio_end: AudioEnd,
    ui_end: Option<UiEnd>,
    inputs: Inputs,
    channel_params: [ChannelParams; NUM_CHANNELS],
    output_slot: usize,
    color: NoiseColor,
    bandwidth: i32,
    stereo: bool,
    amount_limit_to: StereoSample,
    amount_limit_slope: StereoSample,
    steal_phase: bool,
    /// `power_scale(color) / π`, shared by every voice.
    magnitude_scale: Sample,
    random: Pcg32,
    /// Harmonic phase in radians, per channel and voice. DC stays at 0.
    phases: VoicesLayout<Box<[Sample; SPECTRAL_BUFFER_SIZE]>>,
    /// `log2` of harmonic indices. Index 0 is unused.
    log2: [Sample; SPECTRAL_BUFFER_SIZE],
    /// Pending note-on. Applied on the voice's next left-channel render.
    phase_reset: [Option<PhaseReset>; MAX_VOICES],
}

impl SpectralNoise {
    pub fn new(id: ModuleId) -> Self {
        Self::from_config(&SpectralNoiseConfig {
            id,
            ..SpectralNoiseConfig::default()
        })
    }

    pub fn from_config(config: &SpectralNoiseConfig) -> Self {
        let (audio_end, ui_end) = create_link_pair();
        let mut noise = Self {
            id: config.id,
            audio_end,
            ui_end: Some(ui_end),
            inputs: Inputs::default(),
            channel_params: array::from_fn(|channel_idx| {
                ChannelParams::from_config(config, channel_idx)
            }),
            output_slot: usize::MAX,
            color: config.color,
            bandwidth: Self::clamp_bandwidth(config.bandwidth),
            stereo: config.stereo,
            amount_limit_to: Self::clamp_amount_limit_to(config.amount_limit_to),
            amount_limit_slope: Self::clamp_amount_limit_slope(config.amount_limit_slope),
            steal_phase: config.steal_phase,
            magnitude_scale: Self::magnitude_scale(config.color),
            random: Pcg32::new(0xa5a5_5a5a_c3c3_3c3c, 0x9e3779b97f4a7c15),
            phases: Box::new(array::from_fn(|_| {
                array::from_fn(|_| Box::new([0.0; SPECTRAL_BUFFER_SIZE]))
            })),
            phase_reset: [None; MAX_VOICES],
            log2: array::from_fn(|harmonic| {
                if harmonic == 0 {
                    0.0
                } else {
                    (harmonic as Sample).log2()
                }
            }),
        };

        for voice_idx in 0..MAX_VOICES {
            noise.randomize_voice_phases(voice_idx);
        }

        noise
    }

    pub fn get_config(&self) -> SpectralNoiseConfig {
        SpectralNoiseConfig {
            id: self.id,
            color: self.color,
            bandwidth: self.bandwidth,
            level: get_stereo_param!(self, level),
            stereo: self.stereo,
            amount: get_stereo_param!(self, amount),
            amount_limit_to: self.amount_limit_to,
            amount_limit_slope: self.amount_limit_slope,
            steal_phase: self.steal_phase,
        }
    }

    pub fn set_color(&mut self, color: NoiseColor) {
        self.color = color;
        self.magnitude_scale = Self::magnitude_scale(color);
    }

    pub fn set_bandwidth(&mut self, bandwidth: i32) {
        self.bandwidth = Self::clamp_bandwidth(bandwidth);
    }

    set_stereo_param!(set_level, level, Self::clamp_level(*level));
    set_stereo_param!(set_amount, amount, Self::clamp_amount(*amount));

    pub fn set_stereo(&mut self, stereo: bool) {
        self.stereo = stereo;
    }

    pub fn set_amount_limit_to(&mut self, value: StereoSample) {
        self.amount_limit_to = Self::clamp_amount_limit_to(value);
    }

    pub fn set_amount_limit_slope(&mut self, value: StereoSample) {
        self.amount_limit_slope = Self::clamp_amount_limit_slope(value);
    }

    pub fn set_steal_phase(&mut self, steal_phase: bool) {
        self.steal_phase = steal_phase;
    }

    fn clamp_bandwidth(bandwidth: i32) -> i32 {
        bandwidth.clamp(0, MAX_BANDWIDTH as i32)
    }

    fn clamp_level(level: Sample) -> Sample {
        level.clamp(MIN_LEVEL_DB, MAX_LEVEL_DB)
    }

    fn clamp_amount(amount: Sample) -> Sample {
        amount.clamp(0.0, 1.0)
    }

    fn clamp_amount_limit_to(value: StereoSample) -> StereoSample {
        value.clamp(MIN_CUTOFF, MAX_CUTOFF)
    }

    fn clamp_amount_limit_slope(value: StereoSample) -> StereoSample {
        value.clamp(0.0, 1.0)
    }

    fn level_gain(level_db: Sample) -> Sample {
        if level_db <= MIN_LEVEL_DB {
            0.0
        } else {
            db_to_gain(level_db)
        }
    }

    /// Unnormalized harmonic weight. White is flat, pink is `1/sqrt(n)`, brown is `1/n`.
    fn color_weight(color: NoiseColor, harmonic: usize) -> Sample {
        let n = harmonic as Sample;

        match color {
            NoiseColor::White => 1.0,
            NoiseColor::Pink => n.sqrt().recip(),
            NoiseColor::Brown => n.recip(),
        }
    }

    /// Scales a color onto brown noise's total power across the full harmonic buffer.
    fn magnitude_scale(color: NoiseColor) -> Sample {
        let mut color_power = 0.0;
        let mut brown_power = 0.0;

        for idx in DC_OFFSET..SPECTRAL_BUFFER_SIZE {
            let weight = Self::color_weight(color, idx);
            color_power += weight * weight;

            let n = idx as Sample;
            brown_power += 1.0 / (n * n);
        }

        (brown_power / color_power).sqrt() / std::f32::consts::PI
    }

    fn randomize_voice_phases(&mut self, voice_idx: usize) {
        for channel in 0..NUM_CHANNELS {
            self.phases[channel][voice_idx][0] = 0.0;

            for phase in self.phases[channel][voice_idx].iter_mut().skip(DC_OFFSET) {
                *phase = self.random.random::<Sample>() * f32::consts::TAU;
            }
        }
    }

    fn copy_voice_phases(&mut self, dst: usize, src: usize) {
        if dst == src {
            return;
        }

        for channel in self.phases.iter_mut() {
            let (src_phases, dst_phases) = if src < dst {
                let (left, right) = channel.split_at_mut(dst);
                (&left[src], &mut right[0])
            } else {
                let (left, right) = channel.split_at_mut(src);
                (&right[0], &mut left[dst])
            };

            dst_phases.copy_from_slice(&src_phases[..]);
        }
    }

    /// On note-on, replace this voice's phases. Steal copies the replaced voice;
    /// otherwise every harmonic gets a new random phase.
    fn apply_phase_reset(&mut self, voice_idx: usize) {
        let Some(reset) = self.phase_reset[voice_idx].take() else {
            return;
        };

        if self.steal_phase
            && let Some(src) = reset.steal_from
            && src != voice_idx
        {
            self.copy_voice_phases(voice_idx, src);
        } else {
            self.randomize_voice_phases(voice_idx);
        }
    }

    fn turn_phase(phase: &mut Sample, max_turn: Sample, rng: &mut Pcg32) {
        let turn = (rng.random::<Sample>() * 2.0 - 1.0) * max_turn;

        *phase = (*phase + turn).rem_euclid(f32::consts::TAU);
    }

    /// Writes this voice's harmonics into `out`. `out.len()` is the bandlimited
    /// count, including the silent DC bin. Phases come from the stored buffer.
    fn write_harmonics(
        &self,
        voice_idx: usize,
        channel: usize,
        gain: Sample,
        out: &mut [ComplexSample],
    ) {
        let Some((dc, harmonics)) = out.split_first_mut() else {
            return;
        };

        *dc = ComplexSample::ZERO;
        let mag_scale = self.magnitude_scale * gain;
        let phases = &self.phases[channel][voice_idx];

        for (offset, bin) in harmonics.iter_mut().enumerate() {
            let idx = offset + DC_OFFSET;
            let magnitude = Self::color_weight(self.color, idx) * mag_scale;

            *bin = ComplexSample::from_polar(magnitude, phases[idx]);
        }
    }

    fn pitch_bandwidth(pitch: Sample) -> usize {
        let frequency = pitch_to_freq(pitch).max(1.0);

        (BAND_LIMIT_FREQUENCY / frequency).floor() as usize
    }

    fn spectrum_length(&self, pitch: Sample) -> usize {
        let bandwidth = if self.bandwidth == 0 {
            Self::pitch_bandwidth(pitch)
        } else {
            self.bandwidth as usize
        };

        (bandwidth + 1).min(SPECTRAL_BUFFER_SIZE)
    }

    fn process_voice(
        &mut self,
        target: &VoiceTarget,
        outputs: &mut VoicesLayout<SpectralOutput>,
        rf: &mut RouterFactory<SpectralRouterType>,
    ) {
        let (mut router, mut voice_output) = rf.for_voice(target, outputs);
        let channel = target.channel_idx;
        let pitch = router
            .direct_opt(self.inputs.pitch)
            .unwrap_or_else(|| target.note_pitch());
        let amount = router
            .scalar(&self.inputs.amount, self.channel_params[channel].amount)
            .clamp(0.0, 1.0);
        let amount = amount * amount;
        let level = router
            .scalar(&self.inputs.level, self.channel_params[channel].level)
            .clamp(MIN_LEVEL_DB, MAX_LEVEL_DB);

        let out = voice_output.output(self.spectrum_length(pitch));
        let length = out.len();
        let voice_idx = target.voice_idx;

        if channel == LEFT_CHANNEL {
            self.apply_phase_reset(voice_idx);
        }

        let gain = Self::level_gain(level);
        let phase_channel = if self.stereo { channel } else { LEFT_CHANNEL };
        let limit_to = self.amount_limit_to[phase_channel];
        let slope = self.amount_limit_slope[phase_channel];

        if self.stereo || channel == LEFT_CHANNEL {
            let phases = &mut self.phases[phase_channel][voice_idx][DC_OFFSET..length];
            let rng = &mut self.random;
            let log2_cutoff = limit_to - (pitch - C4_PITCH);
            let cutoff = log2_cutoff.exp2();
            let slope = slope.clamp(0.0, 1.0);
            let split = (cutoff.ceil() as usize)
                .saturating_sub(DC_OFFSET)
                .min(phases.len());
            let (limited, full) = phases.split_at_mut(split);
            let full_turn = f32::consts::PI * amount;

            if slope < 1e-5 {
                for (offset, phase) in limited.iter_mut().enumerate() {
                    let scale = (DC_OFFSET + offset) as Sample / cutoff;

                    Self::turn_phase(phase, full_turn * scale, rng);
                }
            } else {
                let rate = 1.0 + slope * MAX_AMOUNT_LIMIT_POWER;

                for (offset, phase) in limited.iter_mut().enumerate() {
                    let harmonic = DC_OFFSET + offset;
                    let scale = (rate * (self.log2[harmonic] - log2_cutoff)).exp2();

                    Self::turn_phase(phase, full_turn * scale, rng);
                }
            }

            for phase in full {
                Self::turn_phase(phase, full_turn, rng);
            }
        } else {
            let [left, right] = self.phases.each_mut();
            right[voice_idx][..length].copy_from_slice(&left[voice_idx][..length]);
        }

        self.write_harmonics(voice_idx, phase_channel, gain, out);

        if channel == LEFT_CHANNEL {
            self.audio_end.update_display_spectrum(out);
        }
    }
}

impl SynthModule for SpectralNoise {
    fn id(&self) -> ModuleId {
        self.id
    }

    fn inputs(&self) -> &'static [InputMeta] {
        static INPUTS: &[InputMeta] = &[
            InputMeta::direct_control(Input::Pitch),
            InputMeta::control(Input::Amount),
            InputMeta::control(Input::Level),
        ];

        INPUTS
    }

    fn output_type(&self) -> DataType {
        DataType::Spectral
    }

    fn output_slot(&self) -> usize {
        self.output_slot
    }

    fn set_output_slot(&mut self, slot: usize) {
        self.output_slot = slot;
    }

    fn set_input_slots(&mut self, inputs: &[InputSlots], _spectral_inputs: &[SpectralInputSlot]) {
        self.inputs = Inputs::from_slots(inputs);
    }

    fn update_input_amount(&mut self, input_type: Input, src_slot: usize, amount: StereoSample) {
        self.inputs.update_amount(input_type, src_slot, amount);
    }

    fn process_events(&mut self, events: &[VoiceEvent]) {
        for event in events {
            if let VoiceEvent::Reset {
                voice_idx,
                replaced_voice_idx,
                ..
            } = event
            {
                self.phase_reset[*voice_idx] = Some(PhaseReset {
                    steal_from: *replaced_voice_idx,
                });
            }
        }
    }

    fn process_ui_events(&mut self) {
        while let Some(event) = self.audio_end.pop_event() {
            match event {
                UiEvent::InputParam { input, value } => match input {
                    Input::Amount => self.set_amount(value),
                    Input::Level => self.set_level(value),
                    _ => (),
                },
                UiEvent::Color(color) => self.set_color(color),
                UiEvent::Bandwidth(bandwidth) => self.set_bandwidth(bandwidth),
                UiEvent::Stereo(stereo) => self.set_stereo(stereo),
                UiEvent::AmountLimitTo(value) => self.set_amount_limit_to(value),
                UiEvent::AmountLimitSlope(value) => self.set_amount_limit_slope(value),
                UiEvent::StealPhase(steal_phase) => self.set_steal_phase(steal_phase),
            }
        }
    }

    fn process(&mut self, ctx: &mut ProcessContext) {
        ctx.spectral(self.id, self.output_slot)
            .for_voices(|rf, target, outputs| {
                self.process_voice(target, outputs, rf);
            });
    }
}
