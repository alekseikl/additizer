use itertools::izip;
use wide::f32x4;

use crate::synth_engine::Sample;

const fn tap(x1: f32, x2: f32) -> f32x4 {
    f32x4::new([x1, x2, x1, x2])
}

const NUM_TAPS: usize = 6;
const CHANNELS: usize = 2;

static TAPS: [f32x4; NUM_TAPS] = [
    tap(0.093_022_42, 0.024_388_384),
    tap(0.312_318_06, 0.194_029_99),
    tap(0.548_379_06, 0.433_855_68),
    tap(0.737_198_53, 0.650_124_97),
    tap(0.872_235, 0.810_418_67),
    tap(0.975_497_8, 0.925_979_7),
];

pub struct IirDecimator {
    in_memory: [f32x4; NUM_TAPS],
    out_memory: [f32x4; NUM_TAPS],
}

impl IirDecimator {
    pub fn new() -> Self {
        Self {
            in_memory: Default::default(),
            out_memory: Default::default(),
        }
    }

    pub fn process(&mut self, input: [&[Sample]; CHANNELS], mut output: [&mut [Sample]; CHANNELS]) {
        let (out_left, out_right) = output.split_at_mut(1);

        for (out_left, out_right, in_left, in_right) in izip!(
            out_left[0].iter_mut(),
            out_right[0].iter_mut(),
            input[0].chunks_exact(2),
            input[1].chunks_exact(2)
        ) {
            let mut result = f32x4::new([in_left[0], in_left[1], in_right[0], in_right[1]]);

            for (in_mem, out_mem, tap) in izip!(
                self.in_memory.iter_mut(),
                self.out_memory.iter_mut(),
                TAPS.iter()
            ) {
                let new_result = tap.mul_add(result - *out_mem, *in_mem);

                *in_mem = result;
                *out_mem = new_result;
                result = new_result;
            }

            let result = result.as_array();

            *out_left = 0.5 * (result[0] + result[1]);
            *out_right = 0.5 * (result[2] + result[3]);
        }
    }
}
