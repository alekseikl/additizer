use crate::synth_engine::{Sample, buffer::SPECTRAL_BUFFER_SIZE};

const HARMONICS_NUM: usize = SPECTRAL_BUFFER_SIZE - 1;

pub enum BiquadFilterType {
    LowPass,
    HighPass,
    BandPass,
    BandStop,
    Peaking,
}

pub struct BiquadFilter {
    gain: Sample,
    cutoff: Sample,
    q: Sample,
}

impl BiquadFilter {
    pub fn new(gain: Sample, cutoff: Sample, q: Sample) -> Self {
        Self { gain, cutoff, q }
    }

    #[inline(always)]
    fn common_denominator(x_squared: Sample, w_q_squared: Sample, first_term: Sample) -> Sample {
        x_squared.mul_add(w_q_squared, first_term * first_term)
    }

    pub fn low_pass_4(&self) -> impl Iterator<Item = Sample> + 'static {
        let w = self.cutoff;
        let w_squared = w * w;
        let w_q = w / self.q;
        let w_q_squared = w_q * w_q;
        let numerator = self.gain * w_squared;
        let numerator_squared = numerator * numerator;

        (0..HARMONICS_NUM).map(move |i| {
            let x = i as Sample;
            let x_squared = x * x;
            let first_term = w_squared - x_squared;

            numerator_squared / Self::common_denominator(x_squared, w_q_squared, first_term)
        })
    }

    pub fn high_pass_4(&self) -> impl Iterator<Item = Sample> + 'static {
        let a = self.gain;
        let w = self.cutoff;
        let w_squared = w * w;
        let w_q = w / self.q;
        let w_q_squared = w_q * w_q;

        (0..HARMONICS_NUM).map(move |i| {
            let x = i as Sample;
            let x_squared = x * x;
            let first_term = w_squared - x_squared;
            let numerator = a * x_squared;

            (numerator * numerator) / Self::common_denominator(x_squared, w_q_squared, first_term)
        })
    }

    pub fn band_pass_4(&self) -> impl Iterator<Item = Sample> + 'static {
        let a = self.gain;
        let w = self.cutoff;
        let q = self.q;
        let w_squared = w * w;
        let w_q = w / q;
        let w_q_squared = w_q * w_q;
        let aw_q = a * w / q;
        let aw_q_squared = aw_q * aw_q;

        (0..HARMONICS_NUM).map(move |i| {
            let x = i as Sample;
            let x_squared = x * x;
            let first_term = w_squared - x_squared;

            (aw_q_squared * x_squared)
                / Self::common_denominator(x_squared, w_q_squared, first_term)
        })
    }

    pub fn band_stop_4(&self) -> impl Iterator<Item = Sample> + 'static {
        let a_abs = self.gain.abs();
        let w = self.cutoff;
        let w_squared = w * w;
        let w_q = w / self.q;
        let w_q_squared = w_q * w_q;

        (0..HARMONICS_NUM).map(move |i| {
            let x = i as Sample;
            let x_squared = x * x;
            let first_term = w_squared - x_squared;
            let numerator = a_abs * first_term.abs();

            (numerator * numerator) / Self::common_denominator(x_squared, w_q_squared, first_term)
        })
    }

    pub fn peaking_4(&self) -> impl Iterator<Item = Sample> + 'static {
        let a = self.gain;
        let w = self.cutoff;
        let q = self.q;
        let w_squared = w * w;
        let aw_q = a * w / q;
        let aw_q_squared = aw_q * aw_q;
        let w_qa = w / (q * a);
        let w_qa_squared = w_qa * w_qa;

        (0..HARMONICS_NUM).map(move |i| {
            let x = i as Sample;
            let x_squared = x * x;
            let first_term = w_squared - x_squared;
            let first_term_squared = first_term * first_term;

            x_squared.mul_add(aw_q_squared, first_term_squared)
                / x_squared.mul_add(w_qa_squared, first_term_squared)
        })
    }

    fn apply_order(
        filter_iter: impl Iterator<Item = Sample> + 'static,
        order: Sample,
    ) -> Box<dyn Iterator<Item = Sample>> {
        let power = order / 4.0;

        Box::new(filter_iter.map(move |magnitude| magnitude.powf(power)))
    }

    pub fn filter_iter(
        &self,
        filter_type: BiquadFilterType,
        order: Sample,
    ) -> Box<dyn Iterator<Item = Sample>> {
        let order = order.clamp(2.0, 8.0);

        match filter_type {
            BiquadFilterType::LowPass => Self::apply_order(self.low_pass_4(), order),
            BiquadFilterType::HighPass => Self::apply_order(self.high_pass_4(), order),
            BiquadFilterType::BandPass => Self::apply_order(self.band_pass_4(), order),
            BiquadFilterType::BandStop => Self::apply_order(self.band_stop_4(), order),
            BiquadFilterType::Peaking => Self::apply_order(self.peaking_4(), order),
        }
    }
}
