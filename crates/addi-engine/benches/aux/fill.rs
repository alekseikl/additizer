use addi_engine::Smp;

pub(super) enum Fill {
    Constant(Smp),
    Samples(Vec<Smp>),
}

impl Fill {
    pub(super) fn write(&self, output: &mut [Smp]) {
        match self {
            Self::Constant(value) => output.fill(*value),
            Self::Samples(values) => {
                let n = values.len().min(output.len());

                output[..n].copy_from_slice(&values[..n]);

                if n < output.len() {
                    output[n..].fill(values[n - 1]);
                }
            }
        }
    }
}
