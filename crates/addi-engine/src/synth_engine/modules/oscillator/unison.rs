use crate::synth_engine::{
    power_scale,
    routing::{EngineAudioEnd, LEFT_CHANNEL},
    types::Sample,
};

use super::{ChannelParams, Inputs, MAX_UNISON_VOICES, Router};

const MAX_DETUNE: Sample = 1.0;
const MAX_DETUNE_POWER: Sample = 5.0;

fn blend(from: Sample, to: Sample, amount: Sample) -> Sample {
    (to - from).mul_add(amount, from)
}

pub(super) struct UnisonStateUpdate {
    pub(super) rate: Sample,
    pub(super) phase_shift: Sample,
    pub(super) gain: Sample,
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

pub(super) struct ManualUnison;

impl ManualUnison {
    fn rate(idx: usize, unison: usize, detune: Sample, detune_power: Sample) -> Sample {
        let center = 0.5 * (unison - 1) as Sample;
        let spread = (idx as Sample - center) * center.recip();

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
    stereo_mult: Sample,
    center_mult: Sample,
    channel_idx: usize,
}

impl FlatUnison {
    pub(super) fn new(stereo: Sample, channel_idx: usize) -> Self {
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
        detune: Sample,
        detune_power: Sample,
        amplitudes: (Sample, Sample),
    ) -> (Sample, Sample) {
        debug_assert!(unison >= 2 && idx < unison);

        let (center_amp, detuned_amp) = amplitudes;

        let span = unison - 1;
        let distance = (2 * idx).abs_diff(span);
        let pair = distance / 2;
        let position = ((2 * idx) as Sample - span as Sample) / span as Sample;

        let curved = power_scale(position.abs(), detune_power).copysign(position);
        let rate = (curved * detune).exp2();

        let center = Sample::from(pair == 0);
        let amp = (center_amp - detuned_amp).mul_add(center, detuned_amp);
        let negative = 2 * idx < span;
        let voice_on_left = negative ^ ((pair & 1) == 1);
        let on_left = self.channel_idx == LEFT_CHANNEL;
        let both = distance == 0;
        let native = Sample::from(both || voice_on_left == on_left);
        let other = Sample::from(both || voice_on_left != on_left);
        let gain = amp * (self.stereo_mult * native + self.center_mult * other);

        (rate, gain)
    }

    fn amplitudes(unison: usize, blend: Sample) -> (Sample, Sample) {
        const CENTER_LOW_AMPLITUDE: Sample = 0.4;
        const DETUNED_HIGH_AMPLITUDE: Sample = 0.6;

        if unison <= 2 {
            return (1.0, 0.0);
        }

        let blend = blend.clamp(0.0, 1.0);
        let center = (CENTER_LOW_AMPLITUDE - 1.0).mul_add(blend, 1.0);
        let detuned_blend = 1.0 - blend;
        let detuned = DETUNED_HIGH_AMPLITUDE * (1.0 - detuned_blend * detuned_blend);
        let pairs = unison.div_ceil(2);
        let square_sums = center * center + detuned * detuned * (pairs - 1) as Sample;

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
    stereo: Sample,
    channel_idx: usize,
}

impl ConvexUnison {
    pub(super) fn new(stereo: Sample, channel_idx: usize) -> Self {
        let stereo = stereo.clamp(0.0, 1.0);

        Self {
            stereo: stereo.sqrt(),
            channel_idx,
        }
    }

    pub(super) fn rate_gain(
        &self,
        idx: usize,
        unison: usize,
        detune: Sample,
        detune_power: Sample,
        adjustment: Sample,
    ) -> (Sample, Sample) {
        debug_assert!(unison >= 2 && idx < unison);

        let span = unison - 1;
        let distance = (2 * idx).abs_diff(span);
        let pair = distance / 2;
        let position = ((2 * idx) as Sample - span as Sample) / span as Sample;
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

    fn shaped_level(amount: Sample, side: Sample) -> Sample {
        const CENTER_AMPLITUDE: Sample = 1.0;

        (side - CENTER_AMPLITUDE).mul_add(amount, CENTER_AMPLITUDE)
    }

    pub(super) fn mono_level(spread: Sample) -> Sample {
        Self::shaped_level(spread * spread, std::f32::consts::FRAC_1_SQRT_2)
    }

    pub(super) fn native_level(spread: Sample) -> Sample {
        Self::shaped_level(spread.sqrt(), std::f32::consts::SQRT_2)
    }

    fn other_level(steps: usize, unison: usize) -> Sample {
        const BASE: Sample = std::f32::consts::FRAC_1_SQRT_2 * std::f32::consts::FRAC_1_SQRT_2;

        if steps == (unison - 1).div_ceil(2) {
            0.0
        } else {
            BASE.powi(steps as i32)
        }
    }

    pub(super) fn adjustment(unison: usize) -> Sample {
        let pairs = unison.div_ceil(2);
        let span = unison - 1;
        let mut square_sums = 0.0;

        for pair in 0..pairs {
            let spread = (2 * pair + (span & 1)) as Sample / span as Sample;
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
