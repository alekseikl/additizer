use crate::ui_bridge::ModuleUiBridge;
use addi_dsp::time_segments::TimeSegments;
use addi_engine::multi_envelope::{
    MultiEnvelope, MultiEnvelopeConfig, MultiEnvelopeLinks, MultiEnvelopePhase, MultiEnvelopeUiEnd,
    UiUpdate,
};

pub struct MultiEnvelopeUiBridge<L: MultiEnvelopeLinks = crate::links::multi_envelope::Links> {
    ui_end: L::UiEnd,
    config: MultiEnvelopeConfig,
}

impl<L: MultiEnvelopeLinks> MultiEnvelopeUiBridge<L> {
    pub fn try_new(envelope: &mut MultiEnvelope<L>) -> Option<Self> {
        Some(Self {
            ui_end: envelope.take_ui_end()?,
            config: envelope.get_config(),
        })
    }

    pub fn config(&self) -> &MultiEnvelopeConfig {
        &self.config
    }

    pub fn segments_mut(&mut self) -> &mut TimeSegments {
        &mut self.config.segments
    }

    pub fn playhead(&mut self) -> MultiEnvelopePhase {
        self.ui_end.get_phase()
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
}

impl<L: MultiEnvelopeLinks> ModuleUiBridge for MultiEnvelopeUiBridge<L> {
    fn update(&mut self) -> bool {
        let mut routing_refresh = false;

        while let Some(update) = self.ui_end.pop_update() {
            match update {
                UiUpdate::RefreshRouting => routing_refresh = true,
            }
        }

        routing_refresh
    }
}
