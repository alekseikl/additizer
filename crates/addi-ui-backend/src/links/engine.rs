use std::collections::VecDeque;

use triple_buffer::triple_buffer;

use addi_engine::{
    Input, InputId, ModuleId, ModuleUiLink, NUM_CHANNELS, Smp, StereoSample,
    UI_TO_AUDIO_RING_CAPACITY,
    engine_io::{
        EngineAudioEnd, EngineUiEnd, OutputMeter, UiEvent, UiUpdate, VoicesStatus, VoicesTelemetry,
    },
    routing_state::RoutingState,
};

use crate::links::PluginLinks;

// Larger than module links: this carries per-input modulation telemetry.
const AUDIO_TO_UI_RING_CAPACITY: usize = 1024;

pub struct AudioEnd {
    rx: rtrb::Consumer<UiEvent>,
    tx: rtrb::Producer<UiUpdate<ModuleUiLink<PluginLinks>>>,
    out_volume: triple_buffer::Input<OutputMeter>,
    routing: triple_buffer::Input<RoutingState>,
    stuck_events: VecDeque<UiUpdate<ModuleUiLink<PluginLinks>>>,
}

impl AudioEnd {
    fn push_update(&mut self, update: UiUpdate<ModuleUiLink<PluginLinks>>) {
        if !self.stuck_events.is_empty() {
            self.stuck_events.push_back(update);
            return;
        }

        if let Err(rtrb::PushError::Full(update)) = self.tx.push(update) {
            self.stuck_events.push_back(update);
        }
    }
}

pub struct UiEnd {
    rx: rtrb::Consumer<UiUpdate<ModuleUiLink<PluginLinks>>>,
    tx: rtrb::Producer<UiEvent>,
    out_volume: triple_buffer::Output<OutputMeter>,
    routing: triple_buffer::Output<RoutingState>,
}

pub fn make_link_pair() -> (AudioEnd, UiEnd) {
    let (to_audio_tx, from_ui_rx) = rtrb::RingBuffer::<UiEvent>::new(UI_TO_AUDIO_RING_CAPACITY);
    let (to_ui_tx, from_audio_rx) =
        rtrb::RingBuffer::<UiUpdate<ModuleUiLink<PluginLinks>>>::new(AUDIO_TO_UI_RING_CAPACITY);
    let (out_volume_input, out_volume_output) = triple_buffer(&OutputMeter::default());
    let (routing_input, routing_output) = triple_buffer(&RoutingState::default());

    (
        AudioEnd {
            rx: from_ui_rx,
            tx: to_ui_tx,
            out_volume: out_volume_input,
            routing: routing_input,
            stuck_events: VecDeque::with_capacity(32),
        },
        UiEnd {
            rx: from_audio_rx,
            tx: to_audio_tx,
            out_volume: out_volume_output,
            routing: routing_output,
        },
    )
}

impl EngineAudioEnd for AudioEnd {
    type ModuleUi = ModuleUiLink<PluginLinks>;

    fn update_modulated_input(
        &mut self,
        module_id: ModuleId,
        input: Input,
        channel: u8,
        value: Smp,
        normalized_value: Smp,
    ) -> bool {
        self.tx
            .push(UiUpdate::ModulatedInput {
                module_id,
                input,
                channel,
                value,
                normalized_value,
            })
            .is_ok()
    }

    fn update_voices_status(&mut self, telemetry: &VoicesTelemetry) -> bool {
        self.tx
            .push(UiUpdate::VoicesStatus(VoicesStatus {
                waiting_notes: telemetry.waiting as u8,
                playing: telemetry.playing as u8,
                releasing: telemetry.releasing as u8,
                killing: telemetry.killing as u8,
            }))
            .is_ok()
    }

    fn pop_event(&mut self) -> Option<UiEvent> {
        self.rx.pop().ok()
    }

    fn update_out_volume(&mut self, volume: StereoSample, clipped: [bool; NUM_CHANNELS]) {
        *self.out_volume.input_buffer_mut() = OutputMeter { volume, clipped };
        self.out_volume.publish();
    }

    fn publish_routing(&mut self, routing: RoutingState) {
        self.routing.write(routing);
    }

    fn publish_routing_update(&mut self) {
        self.push_update(UiUpdate::Routing);
    }

    fn publish_module_added(&mut self, module: Self::ModuleUi, tag: Option<i32>) {
        self.push_update(UiUpdate::ModuleAdded { module, tag });
    }

    fn flush_stuck_events(&mut self) {
        while let Some(update) = self.stuck_events.pop_front() {
            if let Err(rtrb::PushError::Full(update)) = self.tx.push(update) {
                self.stuck_events.push_front(update);
                break;
            }
        }
    }

    fn publish_module_removed(&mut self, module_id: ModuleId) {
        self.push_update(UiUpdate::ModuleRemoved(module_id));
    }
}

impl EngineUiEnd for UiEnd {
    type ModuleUi = ModuleUiLink<PluginLinks>;

    fn get_out_volume(&mut self) -> OutputMeter {
        *self.out_volume.read()
    }

    fn set_link_amount(&mut self, src: ModuleId, dst: InputId, amount: StereoSample) -> bool {
        self.tx
            .push(UiEvent::LinkAmount { src, dst, amount })
            .is_ok()
    }

    fn set_voices(&mut self, voices: usize) -> bool {
        self.tx.push(UiEvent::Voices(voices)).is_ok()
    }

    fn set_legato(&mut self, legato: bool) -> bool {
        self.tx.push(UiEvent::Legato(legato)).is_ok()
    }

    fn set_kill_same_note(&mut self, kill_same_note: bool) -> bool {
        self.tx.push(UiEvent::KillSameNote(kill_same_note)).is_ok()
    }

    fn set_voice_kill_time(&mut self, voice_kill_time: Smp) -> bool {
        self.tx
            .push(UiEvent::VoiceKillTime(voice_kill_time))
            .is_ok()
    }

    fn set_output_gain(&mut self, output_gain: StereoSample) -> bool {
        self.tx.push(UiEvent::OutputGain(output_gain)).is_ok()
    }

    fn send_event(&mut self, event: UiEvent) -> bool {
        self.tx.push(event).is_ok()
    }

    fn read_routing(&mut self) -> &RoutingState {
        self.routing.read()
    }

    fn pop_update(&mut self) -> Option<UiUpdate<Self::ModuleUi>> {
        self.rx.pop().ok()
    }
}
