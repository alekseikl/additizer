pub struct NthElement {
    mul: isize,
    add: isize,
    inverted: bool,
}

impl NthElement {
    pub fn new(mul: isize, add: isize, inverted: bool) -> Self {
        Self { mul, add, inverted }
    }

    pub fn matches(&self, harmonic: usize) -> bool {
        let harmonic = harmonic as isize;
        let result = if self.mul == 0 {
            harmonic == self.add
        } else {
            let diff = harmonic - self.add;
            diff % self.mul == 0 && diff / self.mul >= 0
        };

        result ^ self.inverted
    }
}
