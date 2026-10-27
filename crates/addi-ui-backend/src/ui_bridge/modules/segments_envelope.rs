use crate::ui_bridge::ModuleUiBridge;
use addi_dsp::segments::Segments;
use addi_engine::segments_envelope::{
    SegmentsEnvelope, SegmentsEnvelopeConfig, SegmentsEnvelopeLinks, SegmentsEnvelopePhase,
    SegmentsEnvelopeUiEnd, Timebase,
};
use addi_engine::{Input, StereoSample};

pub struct SegmentsEnvelopeUiBridge<
    L: SegmentsEnvelopeLinks = crate::links::segments_envelope::Links,
> {
    ui_end: L::UiEnd,
    config: SegmentsEnvelopeConfig,
}

impl<L: SegmentsEnvelopeLinks> SegmentsEnvelopeUiBridge<L> {
    pub fn try_new(envelope: &mut SegmentsEnvelope<L>) -> Option<Self> {
        Some(Self {
            ui_end: envelope.take_ui_end()?,
            config: envelope.get_config(),
        })
    }

    pub fn config(&self) -> &SegmentsEnvelopeConfig {
        &self.config
    }

    pub fn segments_mut(&mut self) -> &mut Segments {
        &mut self.config.segments
    }

    pub fn playhead(&mut self) -> SegmentsEnvelopePhase {
        self.ui_end.get_phase()
    }

    pub fn set_param(&mut self, input: Input, value: StereoSample) {
        if !self.ui_end.set_param(input, value) {
            return;
        }

        match input {
            Input::Rate => self.config.rate = value,
            Input::PhaseShift => self.config.phase_shift = value,
            _ => (),
        }
    }

    /// Publish the curve from [`segments_mut`](Self::segments_mut) to the audio thread.
    pub fn sync_segments(&mut self) {
        let _ = self.ui_end.set_segments(self.config.segments.clone());
    }

    pub fn set_steal_level(&mut self, value: bool) {
        if self.ui_end.set_steal_level(value) {
            self.config.steal_level = value;
        }
    }

    pub fn set_keep_voice_alive(&mut self, value: bool) {
        if self.ui_end.set_keep_voice_alive(value) {
            self.config.keep_voice_alive = value;
        }
    }

    pub fn set_timebase(&mut self, timebase: Timebase) {
        if self.ui_end.set_timebase(timebase) {
            self.config.timebase = timebase;
        }
    }
}

impl<L: SegmentsEnvelopeLinks> ModuleUiBridge for SegmentsEnvelopeUiBridge<L> {
    fn update(&mut self) -> bool {
        false
    }
}
