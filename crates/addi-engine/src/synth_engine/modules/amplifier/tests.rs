use super::{Amplifier, AmplifierLinks};

impl<L: AmplifierLinks> Amplifier<L> {
    pub(crate) fn audio_slot(&self) -> Option<usize> {
        self.inputs.audio
    }
}
