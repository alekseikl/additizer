use triple_buffer::triple_buffer;

use crate::synth_engine::{
    Input, InputId, ModuleId, NUM_CHANNELS, Sample, StereoSample, UI_TO_AUDIO_RING_CAPACITY,
    engine_io::{
        EngineAudioEnd, EngineUiEnd, OutputMeter, UiEvent, UiUpdate, VoicesHandlerMetrics,
        VoicesStatus,
    },
};

// Larger than module links: this carries per-input modulation telemetry.
const AUDIO_TO_UI_RING_CAPACITY: usize = 1024;

pub struct AudioEnd {
    rx: rtrb::Consumer<UiEvent>,
    tx: rtrb::Producer<UiUpdate>,
    out_volume: triple_buffer::Input<OutputMeter>,
}

impl AudioEnd {
    pub fn update_modulated_input(
        &mut self,
        module_id: ModuleId,
        input: Input,
        channel: u8,
        value: Sample,
        normalized_value: Sample,
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

    pub fn update_voices_status(&mut self, d: &VoicesHandlerMetrics) -> bool {
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
}

pub struct UiEnd {
    rx: rtrb::Consumer<UiUpdate>,
    tx: rtrb::Producer<UiEvent>,
    out_volume: triple_buffer::Output<OutputMeter>,
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

    pub fn set_block_size(&mut self, block_size: usize) -> bool {
        self.tx.push(UiEvent::BlockSize(block_size)).is_ok()
    }

    pub fn set_voice_kill_time(&mut self, voice_kill_time: Sample) -> bool {
        self.tx
            .push(UiEvent::VoiceKillTime(voice_kill_time))
            .is_ok()
    }

    pub fn set_oversampling(&mut self, oversampling: bool) -> bool {
        self.tx.push(UiEvent::Oversampling(oversampling)).is_ok()
    }

    pub fn set_output_gain(&mut self, output_gain: StereoSample) -> bool {
        self.tx.push(UiEvent::OutputGain(output_gain)).is_ok()
    }

    pub fn pop_update(&mut self) -> Option<UiUpdate> {
        self.rx.pop().ok()
    }
}

pub fn make_link_pair() -> (AudioEnd, UiEnd) {
    let (to_audio_tx, from_ui_rx) = rtrb::RingBuffer::<UiEvent>::new(UI_TO_AUDIO_RING_CAPACITY);
    let (to_ui_tx, from_audio_rx) = rtrb::RingBuffer::<UiUpdate>::new(AUDIO_TO_UI_RING_CAPACITY);
    let (out_volume_input, out_volume_output) = triple_buffer(&OutputMeter::default());

    (
        AudioEnd {
            rx: from_ui_rx,
            tx: to_ui_tx,
            out_volume: out_volume_input,
        },
        UiEnd {
            rx: from_audio_rx,
            tx: to_audio_tx,
            out_volume: out_volume_output,
        },
    )
}

impl EngineAudioEnd for AudioEnd {
    fn update_modulated_input(
        &mut self,
        module_id: ModuleId,
        input: Input,
        channel: u8,
        value: Sample,
        normalized_value: Sample,
    ) -> bool {
        AudioEnd::update_modulated_input(self, module_id, input, channel, value, normalized_value)
    }

    fn update_voices_status(&mut self, metrics: &VoicesHandlerMetrics) -> bool {
        AudioEnd::update_voices_status(self, metrics)
    }

    fn pop_event(&mut self) -> Option<UiEvent> {
        AudioEnd::pop_event(self)
    }

    fn update_out_volume(&mut self, volume: StereoSample, clipped: [bool; NUM_CHANNELS]) {
        AudioEnd::update_out_volume(self, volume, clipped)
    }
}

impl EngineUiEnd for UiEnd {
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

    fn set_block_size(&mut self, block_size: usize) -> bool {
        UiEnd::set_block_size(self, block_size)
    }

    fn set_voice_kill_time(&mut self, voice_kill_time: Sample) -> bool {
        UiEnd::set_voice_kill_time(self, voice_kill_time)
    }

    fn set_oversampling(&mut self, oversampling: bool) -> bool {
        UiEnd::set_oversampling(self, oversampling)
    }

    fn set_output_gain(&mut self, output_gain: StereoSample) -> bool {
        UiEnd::set_output_gain(self, output_gain)
    }

    fn pop_update(&mut self) -> Option<UiUpdate> {
        UiEnd::pop_update(self)
    }
}
