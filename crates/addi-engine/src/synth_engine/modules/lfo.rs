use std::array;

use itertools::izip;
use wide::f32x4;

mod config;
mod line;
mod link;
pub mod stub;

#[cfg(test)]
mod tests;

pub use config::LfoConfig;
pub use line::{LfoLine, LfoPoint, MAX_POINTS, MAX_POWER};
pub use link::{LfoAudioEnd, LfoLinks, LfoUiEnd, UiEvent};

use crate::synth_engine::{
    Input, ModuleId, SmoothedSampleParams, Smp, StereoSample,
    buffer::{Buffer, VoicesLayout, new_voices_layout, zero_buffer},
    fast_math::{CATMULL_TAPS, catmull_rom},
    phase::Phase,
    routing::{
        ControlRouterType, DataType, InputMeta, InputSlots, MixedSlots, ProcessContext,
        RouterFactory, SamplesOutput, VoiceEvent, VoiceTarget,
    },
    smooth::SmoothedSample,
    synth_module::SynthModule,
    voices::NUM_CHANNELS,
};

/// One cycle. Matches the oscillator table so [`Phase::wave_index`] lines up.
const WAVEFORM_BITS: usize = 11;
const WAVEFORM_SIZE: usize = 1 << WAVEFORM_BITS;
const WAVEFORM_PAD_LEFT: usize = 1;
const WAVEFORM_PAD_RIGHT: usize = 2;
const WAVEFORM_BUFFER_SIZE: usize = WAVEFORM_SIZE + WAVEFORM_PAD_LEFT + WAVEFORM_PAD_RIGHT;

const _: () = assert!(WAVEFORM_SIZE == 2048);
const _: () = assert!(WAVEFORM_BUFFER_SIZE == WAVEFORM_SIZE - 1 + CATMULL_TAPS);

struct ChannelParams {
    frequency: SmoothedSample,
    phase_shift: SmoothedSample,
}

impl ChannelParams {
    fn from_config(c: &config::LfoConfig, channel_idx: usize) -> Self {
        Self {
            frequency: c.frequency[channel_idx].into(),
            phase_shift: c.phase_shift[channel_idx].into(),
        }
    }

    pub fn advance_smoothers(&mut self, smooth_params: &SmoothedSampleParams, samples: usize) {
        self.frequency.advance(smooth_params, samples);
        self.phase_shift.advance(smooth_params, samples);
    }
}

struct Params {
    line: LfoLine,
    steal_phase: bool,
}

impl Params {
    fn from_config(c: &config::LfoConfig) -> Self {
        Self {
            line: c.line,
            steal_phase: c.steal_phase,
        }
    }
}

#[derive(Default)]
struct VoiceState {
    phase: Phase,
}

pub struct Inputs {
    frequency: MixedSlots,
    phase_shift: MixedSlots,
}

impl Default for Inputs {
    fn default() -> Self {
        Self {
            frequency: MixedSlots::new(Input::LowFrequency),
            phase_shift: MixedSlots::new(Input::PhaseShift),
        }
    }
}

impl Inputs {
    fn from_slots(inputs: &[InputSlots]) -> Self {
        let mut result = Self::default();

        for input in inputs {
            let InputSlots::Mixed(input) = input else {
                continue;
            };

            match input.input_type {
                Input::LowFrequency => result.frequency = input.clone(),
                Input::PhaseShift => result.phase_shift = input.clone(),
                _ => (),
            }
        }

        result
    }

    fn update_amount(&mut self, input_type: Input, src_slot: usize, amount: StereoSample) {
        match input_type {
            Input::LowFrequency => self.frequency.update_amount(src_slot, amount),
            Input::PhaseShift => self.phase_shift.update_amount(src_slot, amount),
            _ => (),
        }
    }
}

struct Buffers {
    frequency: Buffer,
    phase_shift: Buffer,
}

impl Buffers {
    fn new(buffer_size: usize) -> Self {
        Self {
            frequency: zero_buffer(buffer_size),
            phase_shift: zero_buffer(buffer_size),
        }
    }
}

pub struct Lfo<L: LfoLinks = stub::Links> {
    id: ModuleId,
    params: Params,
    channel_params: [ChannelParams; NUM_CHANNELS],
    /// One cycle of [`LfoLine`], padded the same way as the oscillator wavetable.
    wave: [Smp; WAVEFORM_BUFFER_SIZE],
    buffers: Buffers,
    audio_end: L::AudioEnd,
    ui_end: Option<L::UiEnd>,
    inputs: Inputs,
    output_slot: usize,
    voices: VoicesLayout<VoiceState>,
}

impl<L: LfoLinks> Lfo<L> {
    pub fn take_ui_end(&mut self) -> Option<L::UiEnd> {
        self.ui_end.take()
    }

    pub fn new(id: ModuleId, buffer_size: usize) -> Self {
        Self::from_config(
            &LfoConfig {
                id,
                ..LfoConfig::default()
            },
            buffer_size,
        )
    }

    pub fn from_config(config: &config::LfoConfig, buffer_size: usize) -> Self {
        let (audio_end, ui_end) = L::create_link_pair();

        let mut lfo = Self {
            id: config.id,
            params: Params::from_config(config),
            channel_params: array::from_fn(|channel_idx| {
                ChannelParams::from_config(config, channel_idx)
            }),
            wave: [0.0; WAVEFORM_BUFFER_SIZE],
            buffers: Buffers::new(buffer_size),
            audio_end,
            ui_end,
            inputs: Inputs::default(),
            output_slot: usize::MAX,
            voices: new_voices_layout(),
        };

        lfo.bake_wave();
        lfo
    }

    pub fn get_config(&self) -> LfoConfig {
        LfoConfig {
            id: self.id,
            steal_phase: self.params.steal_phase,
            frequency: get_smoothed_param!(self, frequency),
            phase_shift: get_smoothed_param!(self, phase_shift),
            line: self.params.line,
        }
    }

    set_mono_param!(set_steal_phase, steal_phase, bool);

    set_smoothed_param!(set_frequency, frequency);
    set_smoothed_param!(set_phase_shift, phase_shift, phase_shift.clamp(-1.0, 1.0));

    pub fn set_line(&mut self, line: LfoLine) {
        if self.params.line == line {
            return;
        }

        self.params.line = line;
        self.bake_wave();
    }

    fn bake_wave(&mut self) {
        let n = WAVEFORM_SIZE as Smp;

        for i in 0..WAVEFORM_SIZE {
            let t = i as Smp / n;
            self.wave[WAVEFORM_PAD_LEFT + i] = self.params.line.value_at(t);
        }

        let last = WAVEFORM_PAD_LEFT + WAVEFORM_SIZE - 1;
        self.wave[0] = self.wave[last];
        self.wave[last + 1] = self.wave[WAVEFORM_PAD_LEFT];
        self.wave[last + 2] = self.wave[WAVEFORM_PAD_LEFT + 1];
    }

    /// Catmull–Rom read, same pad and weights as the oscillator wavetable.
    #[inline(always)]
    fn read_wave(wave: &[Smp], phase: Phase) -> Smp {
        let idx = phase.wave_index::<WAVEFORM_BITS>();
        let t = phase.wave_index_fraction::<WAVEFORM_BITS>();
        let s = &wave[idx..idx + CATMULL_TAPS];
        let samples = f32x4::new([s[0], s[1], s[2], s[3]]);
        let [a, b, c, d] = (catmull_rom(t) * samples).to_array();

        (a + c) + (b + d)
    }

    fn process_voice(
        &mut self,
        target: &VoiceTarget,
        outputs: &mut VoicesLayout<SamplesOutput>,
        rf: &mut RouterFactory<ControlRouterType, L::EngineEnd>,
    ) {
        let channel_idx = target.channel_idx;
        let voice_idx = target.voice_idx;
        let (mut router, mut voice_output) = rf.for_voice(target, outputs);
        let inputs = &self.inputs;
        let channel = &mut self.channel_params[channel_idx];
        let voice = self.voices.at_mut(channel_idx, voice_idx);
        let sample_rate = router.sample_rate();

        router.param(
            &inputs.frequency,
            &channel.frequency,
            &mut self.buffers.frequency,
        );
        router.param(
            &inputs.phase_shift,
            &channel.phase_shift,
            &mut self.buffers.phase_shift,
        );

        let freq_phase_mult = Phase::freq_phase_mult(sample_rate);
        let wave = self.wave.as_slice();

        for (out, frequency, phase_shift) in izip!(
            voice_output.output().iter_mut(),
            &self.buffers.frequency,
            &self.buffers.phase_shift,
        ) {
            let read = voice.phase.add_normalized(phase_shift.clamp(-1.0, 1.0));

            *out = Self::read_wave(wave, read);

            voice.phase += *frequency * freq_phase_mult;
        }

        if router.need_update_ui_mono() {
            self.audio_end.update_phase(voice.phase.normalized());
        }
    }

    pub(crate) fn process(&mut self, ctx: &mut ProcessContext<L::EngineEnd>) {
        ctx.control(self.id, self.output_slot)
            .for_voices(|rf, target, outputs| {
                self.process_voice(target, outputs, rf);
            })
            .for_channels(|rf, channel_idx| {
                self.channel_params[channel_idx]
                    .advance_smoothers(&rf.params().smooth_params, rf.params().samples);
            });
    }
}

impl<L: LfoLinks> SynthModule for Lfo<L> {
    fn id(&self) -> ModuleId {
        self.id
    }

    fn inputs(&self) -> &'static [InputMeta] {
        static INPUTS: &[InputMeta] = &[
            InputMeta::control(Input::LowFrequency),
            InputMeta::control(Input::PhaseShift),
        ];

        INPUTS
    }

    fn output_type(&self) -> DataType {
        DataType::Control
    }

    fn output_slot(&self) -> usize {
        self.output_slot
    }

    fn set_output_slot(&mut self, slot: usize) {
        self.output_slot = slot;
    }

    fn set_input_slots(&mut self, inputs: &[InputSlots]) {
        self.inputs = Inputs::from_slots(inputs);
    }

    fn update_input_amount(&mut self, input_type: Input, src_slot: usize, amount: StereoSample) {
        self.inputs.update_amount(input_type, src_slot, amount);
    }

    fn process_events(&mut self, events: &[VoiceEvent]) {
        for channel_idx in 0..NUM_CHANNELS {
            for event in events {
                if let VoiceEvent::Reset {
                    voice_idx,
                    replaced_voice_idx,
                    ..
                } = event
                {
                    let phase = if let Some(replaced_voice_idx) = replaced_voice_idx
                        && self.params.steal_phase
                    {
                        self.voices.at(channel_idx, *replaced_voice_idx).phase
                    } else {
                        Phase::ZERO
                    };

                    self.voices.at_mut(channel_idx, *voice_idx).phase = phase;
                }
            }
        }
    }

    fn process_ui_events(&mut self) {
        while let Some(event) = self.audio_end.pop_event() {
            match event {
                UiEvent::InputParam { input, value } => match input {
                    Input::LowFrequency => self.set_frequency(value),
                    Input::PhaseShift => self.set_phase_shift(value),
                    _ => (),
                },
                UiEvent::Line(line) => self.set_line(line),
                UiEvent::StealPhase(value) => self.set_steal_phase(value),
            }
        }
    }
}
