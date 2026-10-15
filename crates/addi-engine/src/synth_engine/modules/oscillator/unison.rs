use crate::synth_engine::{
    power_scale,
    routing::{EngineAudioEnd, LEFT_CHANNEL},
    types::Smp,
};

use super::{ChannelParams, Inputs, MAX_UNISON_VOICES, Router};

const MAX_DETUNE: Smp = 1.0;
const MAX_DETUNE_POWER: Smp = 5.0;
pub struct Interpolated {
    pub from: Smp,
    pub to: Smp,
}

impl Interpolated {
    #[inline(always)]
    pub fn advance(&mut self) {
        self.from = self.to;
    }
}

pub struct UnisonVoice {
    pub rate: Interpolated,
    pub phase_shift: Interpolated,
    pub gain: Interpolated,
}

impl Default for UnisonVoice {
    fn default() -> Self {
        Self {
            rate: Interpolated { from: 1.0, to: 1.0 },
            phase_shift: Interpolated { from: 0.0, to: 0.0 },
            gain: Interpolated { from: 1.0, to: 1.0 },
        }
    }
}

pub(super) struct UnisonStateUpdate {
    pub(super) rate: Smp,
    pub(super) phase_shift: Smp,
    pub(super) gain: Smp,
}

pub(super) trait UnisonType {
    fn process<'a, A: EngineAudioEnd>(
        &self,
        this_frame: bool,
        unison: usize,
        channel: &'a ChannelParams,
        inputs: &Inputs,
        router: &mut Router<'_, '_, '_, A>,
    ) -> impl Iterator<Item = UnisonStateUpdate> + 'a;
}

fn blend(from: Smp, to: Smp, amount: Smp) -> Smp {
    (to - from).mul_add(amount, from)
}

pub(super) struct ManualUnison;

impl ManualUnison {
    fn rate(idx: usize, unison: usize, detune: Smp, detune_power: Smp) -> Smp {
        let center = 0.5 * (unison - 1) as Smp;
        let spread = (idx as Smp - center) * center.recip();

        (power_scale(spread.abs(), detune_power).copysign(spread) * detune).exp2()
    }
}

impl UnisonType for ManualUnison {
    fn process<'a, A: EngineAudioEnd>(
        &self,
        this_frame: bool,
        unison: usize,
        channel: &'a ChannelParams,
        inputs: &Inputs,
        router: &mut Router<'_, '_, '_, A>,
    ) -> impl Iterator<Item = UnisonStateUpdate> + 'a {
        let detune = router
            .scalar(&inputs.detune, channel.detune, this_frame)
            .clamp(0.0, MAX_DETUNE);
        let detune_power = router
            .scalar(&inputs.detune_focus, channel.detune_focus, this_frame)
            .clamp(-1.0, 1.0)
            * MAX_DETUNE_POWER;
        let phases_blend = router
            .scalar(&inputs.phases_blend, channel.phases_blend, this_frame)
            .clamp(0.0, 1.0);
        let gains_blend = router
            .scalar(&inputs.gains_blend, channel.gains_blend, this_frame)
            .clamp(0.0, 1.0);

        let mut gains = [0.0; MAX_UNISON_VOICES];
        let mut sum_sq = 0.0;

        for (gain, param) in gains.iter_mut().zip(channel.unison.iter().take(unison)) {
            let blended = blend(param.gain, param.gain_to, gains_blend);
            *gain = blended;
            sum_sq += blended * blended;
        }
        // Don't amplify a hand-drawn curve whose voices already sum below unity.
        let gain_scale = sum_sq.max(0.0).sqrt().max(1.0).recip();

        channel
            .unison
            .iter()
            .zip(gains)
            .take(unison)
            .enumerate()
            .map(move |(idx, (param, gain))| UnisonStateUpdate {
                rate: Self::rate(idx, unison, detune, detune_power),
                phase_shift: blend(param.phase_shift, param.phase_shift_to, phases_blend),
                gain: gain * gain_scale,
            })
    }
}

#[derive(Clone, Copy)]
pub(super) struct FlatUnison {
    stereo_mult: Smp,
    center_mult: Smp,
    channel_idx: usize,
}

impl FlatUnison {
    pub(super) fn new(stereo: Smp, channel_idx: usize) -> Self {
        let fade = stereo.clamp(0.0, 1.0).mul_add(0.5, 0.5);
        let angle = fade * std::f32::consts::FRAC_PI_2;
        let (stereo_mult, center_mult) = angle.sin_cos();

        Self {
            stereo_mult,
            center_mult,
            channel_idx,
        }
    }

    pub(super) fn rate_gain(
        &self,
        idx: usize,
        unison: usize,
        detune: Smp,
        detune_power: Smp,
        amplitudes: (Smp, Smp),
    ) -> (Smp, Smp) {
        debug_assert!(unison >= 2 && idx < unison);

        let (center_amp, detuned_amp) = amplitudes;

        let span = unison - 1;
        let distance = (2 * idx).abs_diff(span);
        let pair = distance / 2;
        let position = ((2 * idx) as Smp - span as Smp) / span as Smp;

        let curved = power_scale(position.abs(), detune_power).copysign(position);
        let rate = (curved * detune).exp2();

        let center = Smp::from(pair == 0);
        let amp = (center_amp - detuned_amp).mul_add(center, detuned_amp);
        let negative = 2 * idx < span;
        let voice_on_left = negative ^ ((pair & 1) == 1);
        let on_left = self.channel_idx == LEFT_CHANNEL;
        let both = distance == 0;
        let native = Smp::from(both || voice_on_left == on_left);
        let other = Smp::from(both || voice_on_left != on_left);
        let gain = amp * (self.stereo_mult * native + self.center_mult * other);

        (rate, gain)
    }

    fn amplitudes(unison: usize, blend: Smp) -> (Smp, Smp) {
        const CENTER_LOW_AMPLITUDE: Smp = 0.4;
        const DETUNED_HIGH_AMPLITUDE: Smp = 0.6;

        if unison <= 2 {
            return (1.0, 0.0);
        }

        let blend = blend.clamp(0.0, 1.0);
        let center = (CENTER_LOW_AMPLITUDE - 1.0).mul_add(blend, 1.0);
        let detuned_blend = 1.0 - blend;
        let detuned = DETUNED_HIGH_AMPLITUDE * (1.0 - detuned_blend * detuned_blend);
        let pairs = unison.div_ceil(2);
        let square_sums = center * center + detuned * detuned * (pairs - 1) as Smp;

        let adjustment = square_sums.sqrt().recip();
        (adjustment * center, adjustment * detuned)
    }
}

impl UnisonType for FlatUnison {
    fn process<'a, A: EngineAudioEnd>(
        &self,
        this_frame: bool,
        unison: usize,
        channel: &'a ChannelParams,
        inputs: &Inputs,
        router: &mut Router<'_, '_, '_, A>,
    ) -> impl Iterator<Item = UnisonStateUpdate> + 'a {
        let detune = router
            .scalar(&inputs.detune, channel.detune, this_frame)
            .clamp(0.0, MAX_DETUNE);
        let detune_power = router
            .scalar(&inputs.detune_focus, channel.detune_focus, this_frame)
            .clamp(-1.0, 1.0)
            * MAX_DETUNE_POWER;
        let phases_blend = router
            .scalar(&inputs.phases_blend, channel.phases_blend, this_frame)
            .clamp(0.0, 1.0);
        let gains_blend = router
            .scalar(&inputs.gains_blend, channel.gains_blend, this_frame)
            .clamp(0.0, 1.0);
        let amplitudes = Self::amplitudes(unison, gains_blend);
        let style = *self;

        channel
            .unison
            .iter()
            .take(unison)
            .enumerate()
            .map(move |(idx, param)| {
                let (rate, gain) = style.rate_gain(idx, unison, detune, detune_power, amplitudes);
                UnisonStateUpdate {
                    rate,
                    phase_shift: blend(param.phase_shift, param.phase_shift_to, phases_blend),
                    gain,
                }
            })
    }
}

#[derive(Clone, Copy)]
pub(super) struct ConvexUnison {
    stereo: Smp,
    channel_idx: usize,
}

impl ConvexUnison {
    pub(super) fn new(stereo: Smp, channel_idx: usize) -> Self {
        let stereo = stereo.clamp(0.0, 1.0);

        Self {
            stereo,
            channel_idx,
        }
    }

    pub(super) fn rate_gain(
        &self,
        idx: usize,
        unison: usize,
        detune: Smp,
        detune_power: Smp,
        adjustment: Smp,
    ) -> (Smp, Smp) {
        debug_assert!(unison >= 2 && idx < unison);

        let span = unison - 1;
        let distance = (2 * idx).abs_diff(span);
        let pair = distance / 2;
        let position = ((2 * idx) as Smp - span as Smp) / span as Smp;
        let spread = position.abs();

        let curved = power_scale(spread, detune_power).copysign(position);
        let rate = (curved * detune).exp2();

        let negative = 2 * idx < span;
        let voice_on_left = negative ^ ((pair & 1) == 0);
        let on_left = self.channel_idx == LEFT_CHANNEL;
        let wide = if voice_on_left == on_left {
            Self::native_level(spread)
        } else {
            Self::other_level(distance.div_ceil(2), unison)
        };
        let mono = Self::mono_level(spread);
        let gain = (wide - mono).mul_add(self.stereo, mono);

        (rate, gain * adjustment)
    }

    fn shaped_level(amount: Smp, side: Smp) -> Smp {
        const CENTER_AMPLITUDE: Smp = 1.0;

        (side - CENTER_AMPLITUDE).mul_add(amount, CENTER_AMPLITUDE)
    }

    pub(super) fn mono_level(spread: Smp) -> Smp {
        Self::shaped_level(spread * spread, std::f32::consts::FRAC_1_SQRT_2)
    }

    pub(super) fn native_level(spread: Smp) -> Smp {
        Self::shaped_level(spread.sqrt(), std::f32::consts::SQRT_2)
    }

    fn other_level(steps: usize, unison: usize) -> Smp {
        const BASE: Smp = std::f32::consts::FRAC_1_SQRT_2 * std::f32::consts::FRAC_1_SQRT_2;

        if steps == (unison - 1).div_ceil(2) {
            0.0
        } else {
            BASE.powi(steps as i32)
        }
    }

    pub(super) fn adjustment(unison: usize) -> Smp {
        let span = unison - 1;
        let mut square_sums = 0.0;

        for idx in 0..unison {
            let spread = ((2 * idx) as Smp - span as Smp).abs() / span as Smp;
            let level = Self::mono_level(spread);

            square_sums += level * level;
        }

        square_sums.sqrt().recip()
    }
}

impl UnisonType for ConvexUnison {
    fn process<'a, A: EngineAudioEnd>(
        &self,
        this_frame: bool,
        unison: usize,
        channel: &'a ChannelParams,
        inputs: &Inputs,
        router: &mut Router<'_, '_, '_, A>,
    ) -> impl Iterator<Item = UnisonStateUpdate> + 'a {
        let detune = router
            .scalar(&inputs.detune, channel.detune, this_frame)
            .clamp(0.0, MAX_DETUNE);
        let detune_power = router
            .scalar(&inputs.detune_focus, channel.detune_focus, this_frame)
            .clamp(-1.0, 1.0)
            * MAX_DETUNE_POWER;
        let phases_blend = router
            .scalar(&inputs.phases_blend, channel.phases_blend, this_frame)
            .clamp(0.0, 1.0);
        let adjustment = Self::adjustment(unison);
        let style = *self;

        channel
            .unison
            .iter()
            .take(unison)
            .enumerate()
            .map(move |(idx, param)| {
                let (rate, gain) = style.rate_gain(idx, unison, detune, detune_power, adjustment);
                UnisonStateUpdate {
                    rate,
                    phase_shift: blend(param.phase_shift, param.phase_shift_to, phases_blend),
                    gain,
                }
            })
    }
}
