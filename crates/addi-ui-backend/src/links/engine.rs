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
}

impl AudioEnd {
    pub fn update_modulated_input(
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

    pub fn update_voices_status(&mut self, d: &VoicesTelemetry) -> bool {
        self.tx
            .push(UiUpdate::VoicesStatus(VoicesStatus {
                waiting_notes: d.waiting as u8,
                playing: d.playing as u8,
                releasing: d.releasing as u8,
                killing: d.killing as u8,
            }))
            .is_ok()
    }

    pub fn pop_event(&mut self) -> Option<UiEvent> {
        self.rx.pop().ok()
    }

    pub fn update_out_volume(&mut self, volume: StereoSample, clipped: [bool; NUM_CHANNELS]) {
        *self.out_volume.input_buffer_mut() = OutputMeter { volume, clipped };
        self.out_volume.publish();
    }

    pub fn publish_routing(&mut self, routing: RoutingState) {
        self.routing.write(routing);
    }

    pub fn publish_topology(
        &mut self,
        added: Option<ModuleUiLink<PluginLinks>>,
        removed: Option<ModuleId>,
        tag: Option<i32>,
    ) -> bool {
        self.tx
            .push(UiUpdate::Topology {
                added,
                removed,
                tag,
            })
            .is_ok()
    }
}

pub struct UiEnd {
    rx: rtrb::Consumer<UiUpdate<ModuleUiLink<PluginLinks>>>,
    tx: rtrb::Producer<UiEvent>,
    out_volume: triple_buffer::Output<OutputMeter>,
    routing: triple_buffer::Output<RoutingState>,
}

impl UiEnd {
    pub fn get_out_volume(&mut self) -> OutputMeter {
        *self.out_volume.read()
    }

    pub fn set_link_amount(&mut self, src: ModuleId, dst: InputId, amount: StereoSample) -> bool {
        self.tx
            .push(UiEvent::LinkAmount { src, dst, amount })
            .is_ok()
    }

    pub fn set_voices(&mut self, voices: usize) -> bool {
        self.tx.push(UiEvent::Voices(voices)).is_ok()
    }

    pub fn set_legato(&mut self, legato: bool) -> bool {
        self.tx.push(UiEvent::Legato(legato)).is_ok()
    }

    pub fn set_kill_same_note(&mut self, kill_same_note: bool) -> bool {
        self.tx.push(UiEvent::KillSameNote(kill_same_note)).is_ok()
    }

    pub fn set_voice_kill_time(&mut self, voice_kill_time: Smp) -> bool {
        self.tx
            .push(UiEvent::VoiceKillTime(voice_kill_time))
            .is_ok()
    }

    pub fn set_output_gain(&mut self, output_gain: StereoSample) -> bool {
        self.tx.push(UiEvent::OutputGain(output_gain)).is_ok()
    }

    pub fn send_event(&mut self, event: UiEvent) -> bool {
        self.tx.push(event).is_ok()
    }

    pub fn read_routing(&mut self) -> &RoutingState {
        self.routing.read()
    }

    pub fn pop_update(&mut self) -> Option<UiUpdate<ModuleUiLink<PluginLinks>>> {
        self.rx.pop().ok()
    }
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
        AudioEnd::update_modulated_input(self, module_id, input, channel, value, normalized_value)
    }

    fn update_voices_status(&mut self, telemetry: &VoicesTelemetry) -> bool {
        AudioEnd::update_voices_status(self, telemetry)
    }

    fn pop_event(&mut self) -> Option<UiEvent> {
        AudioEnd::pop_event(self)
    }

    fn update_out_volume(&mut self, volume: StereoSample, clipped: [bool; NUM_CHANNELS]) {
        AudioEnd::update_out_volume(self, volume, clipped)
    }

    fn publish_routing(&mut self, routing: RoutingState) {
        AudioEnd::publish_routing(self, routing)
    }

    fn publish_topology(
        &mut self,
        added: Option<Self::ModuleUi>,
        removed: Option<ModuleId>,
        tag: Option<i32>,
    ) -> bool {
        AudioEnd::publish_topology(self, added, removed, tag)
    }
}

impl EngineUiEnd for UiEnd {
    type ModuleUi = ModuleUiLink<PluginLinks>;

    fn get_out_volume(&mut self) -> OutputMeter {
        UiEnd::get_out_volume(self)
    }

    fn set_link_amount(&mut self, src: ModuleId, dst: InputId, amount: StereoSample) -> bool {
        UiEnd::set_link_amount(self, src, dst, amount)
    }

    fn set_voices(&mut self, voices: usize) -> bool {
        UiEnd::set_voices(self, voices)
    }

    fn set_legato(&mut self, legato: bool) -> bool {
        UiEnd::set_legato(self, legato)
    }

    fn set_kill_same_note(&mut self, kill_same_note: bool) -> bool {
        UiEnd::set_kill_same_note(self, kill_same_note)
    }

    fn set_voice_kill_time(&mut self, voice_kill_time: Smp) -> bool {
        UiEnd::set_voice_kill_time(self, voice_kill_time)
    }

    fn set_output_gain(&mut self, output_gain: StereoSample) -> bool {
        UiEnd::set_output_gain(self, output_gain)
    }

    fn send_event(&mut self, event: UiEvent) -> bool {
        UiEnd::send_event(self, event)
    }

    fn read_routing(&mut self) -> &RoutingState {
        UiEnd::read_routing(self)
    }

    fn pop_update(&mut self) -> Option<UiUpdate<Self::ModuleUi>> {
        UiEnd::pop_update(self)
    }
}
