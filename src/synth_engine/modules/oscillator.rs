use std::{any::Any, f32, sync::Arc};

use itertools::izip;
use realfft::{ComplexToReal, RealFftPlanner};
use serde::{Deserialize, Serialize};

use crate::{
    synth_engine::{
        buffer::{
            Buffer, ONES_BUFFER, SpectralBuffer, WAVEFORM_BITS, ZEROES_BUFFER,
            ZEROES_SPECTRAL_BUFFER, make_zero_buffer, make_zero_spectral_buffer,
        },
        routing::{
            InputType, MAX_VOICES, ModuleId, ModuleInput, ModuleType, NUM_CHANNELS, OutputType,
            Router,
        },
        synth_module::{ModuleConfigBox, NoteOnParams, ProcessParams, SynthModule},
        types::{ComplexSample, Phase, Sample, StereoSample},
    },
    utils::{note_to_octave, octave_to_freq, st_to_octave},
};
use uniform_cubic_splines::{CatmullRom, spline_segment};

const WAVEFORM_SIZE: usize = 1 << WAVEFORM_BITS;
const WAVEFORM_PAD_LEFT: usize = 1;
const WAVEFORM_PAD_RIGHT: usize = 2;
const WAVEFORM_BUFFER_SIZE: usize = WAVEFORM_SIZE + WAVEFORM_PAD_LEFT + WAVEFORM_PAD_RIGHT;

const FULL_PHASE: Sample = ((u32::MAX as u64) + 1) as Sample;
const INTERMEDIATE_BITS: usize = 32 - WAVEFORM_BITS;
const INTERMEDIATE_MASK: u32 = (1 << INTERMEDIATE_BITS) - 1;
const INTERMEDIATE_MULT: Sample = ((1 << INTERMEDIATE_BITS) as Sample).recip();
const MAX_UNISON_VOICES: usize = 16;

const INITIAL_PHASES: [Sample; MAX_UNISON_VOICES] = [
    0.46912605, 0.9068176, 0.6544455, 0.26577616, 0.24667478, 0.12834072, 0.5805929, 0.55541587,
    0.58291245, 0.03298676, 0.8845756, 0.96093744, 0.42001683, 0.63606197, 0.28810132, 0.5167134,
];

type WaveformBuffer = [Sample; WAVEFORM_BUFFER_SIZE];

const fn make_zero_wave_buffer() -> WaveformBuffer {
    [0.0; WAVEFORM_BUFFER_SIZE]
}

#[derive(Clone, Serialize, Deserialize)]
pub struct OscillatorConfigChannel {
    level: Sample,
    pitch_shift: Sample,
    detune: Sample,
    phase_shift: Sample,
    initial_phases: [Sample; MAX_UNISON_VOICES],
}

impl Default for OscillatorConfigChannel {
    fn default() -> Self {
        Self {
            level: 1.0,
            pitch_shift: 0.0,
            detune: st_to_octave(0.2),
            phase_shift: 0.0,
            initial_phases: INITIAL_PHASES,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct OscillatorConfig {
    label: Option<String>,
    unison: usize,
    channels: [OscillatorConfigChannel; NUM_CHANNELS],
}

impl Default for OscillatorConfig {
    fn default() -> Self {
        Self {
            label: None,
            unison: 1,
            channels: Default::default(),
        }
    }
}

pub struct OscillatorUIData {
    pub label: String,
    pub level: StereoSample,
    pub pitch_shift: StereoSample,
    pub detune: StereoSample,
    pub phase_shift: StereoSample,
    pub unison: usize,
    pub initial_phases: [StereoSample; MAX_UNISON_VOICES],
}

struct Voice {
    octave: Sample,
    wave_buffers_swapped: bool,
    phases: [Phase; MAX_UNISON_VOICES],
    output: Buffer,
    wave_buffers: (WaveformBuffer, WaveformBuffer),
}

impl Default for Voice {
    fn default() -> Self {
        Self {
            octave: 0.0,
            wave_buffers_swapped: false,
            phases: Default::default(),
            output: make_zero_buffer(),
            wave_buffers: (make_zero_wave_buffer(), make_zero_wave_buffer()),
        }
    }
}

struct Channel {
    level: Sample,
    pitch_shift: Sample, //Octaves
    detune: Sample,      //Octaves
    phase_shift: Sample,
    initial_phases: [Sample; MAX_UNISON_VOICES],
    voices: [Voice; MAX_VOICES],
}

impl Default for Channel {
    fn default() -> Self {
        Self {
            level: 0.5,
            pitch_shift: 0.0,
            detune: st_to_octave(0.3),
            phase_shift: 0.0,
            initial_phases: INITIAL_PHASES,
            voices: Default::default(),
        }
    }
}

struct Common {
    id: ModuleId,
    label: String,
    config: ModuleConfigBox<OscillatorConfig>,
    unison: usize,
    inverse_fft: Arc<dyn ComplexToReal<Sample>>,
    tmp_spectral_buff: SpectralBuffer,
    scratch_buff: SpectralBuffer,
    level_mod_input: Buffer,
    pitch_shift_input: Buffer,
    phase_shift_input: Buffer,
    detune_mod_input: Buffer,
}
pub struct Oscillator {
    common: Common,
    channels: [Channel; NUM_CHANNELS],
}

macro_rules! set_param_method {
    ($fn_name:ident, $param:ident, $transform:expr) => {
        pub fn $fn_name(&mut self, $param: StereoSample) {
            for (channel, $param) in self.channels.iter_mut().zip($param.iter()) {
                channel.$param = $transform;
            }

            {
                let mut cfg = self.common.config.lock();
                for (channel_cfg, channel) in cfg.channels.iter_mut().zip(self.channels.iter()) {
                    channel_cfg.$param = channel.$param;
                }
            }
        }
    };
}

macro_rules! extract_param {
    ($self:ident, $param:ident) => {
        StereoSample::from_iter($self.channels.iter().map(|channel| channel.$param))
    };
}

impl Oscillator {
    pub fn new(id: ModuleId, config: ModuleConfigBox<OscillatorConfig>) -> Self {
        let mut osc = Self {
            common: Common {
                id,
                label: format!("Oscillator {id}"),
                config,
                unison: 1,
                inverse_fft: RealFftPlanner::<Sample>::new().plan_fft_inverse(WAVEFORM_SIZE),
                tmp_spectral_buff: make_zero_spectral_buffer(),
                scratch_buff: make_zero_spectral_buffer(),
                level_mod_input: make_zero_buffer(),
                pitch_shift_input: make_zero_buffer(),
                phase_shift_input: make_zero_buffer(),
                detune_mod_input: make_zero_buffer(),
            },
            channels: Default::default(),
        };

        {
            let cfg = osc.common.config.lock();

            if let Some(label) = cfg.label.as_ref() {
                osc.common.label = label.clone();
            }

            for (channel, cfg_channel) in osc.channels.iter_mut().zip(cfg.channels.iter()) {
                channel.level = cfg_channel.level;
                channel.pitch_shift = cfg_channel.pitch_shift;
                channel.phase_shift = cfg_channel.phase_shift;
                channel.detune = cfg_channel.detune;
                channel.initial_phases = cfg_channel.initial_phases;
            }
            osc.common.unison = cfg.unison;
        }

        osc
    }

    gen_downcast_methods!(Oscillator);

    pub fn get_ui(&self) -> OscillatorUIData {
        OscillatorUIData {
            label: self.common.label.clone(),
            level: extract_param!(self, level),
            pitch_shift: extract_param!(self, pitch_shift),
            detune: extract_param!(self, detune),
            phase_shift: extract_param!(self, phase_shift),
            unison: self.common.unison,
            initial_phases: std::array::from_fn(|i| {
                StereoSample::from_iter(
                    self.channels
                        .iter()
                        .map(|channel| channel.initial_phases[i]),
                )
            }),
        }
    }

    pub fn set_unison(&mut self, unison: usize) {
        self.common.unison = unison.clamp(1, MAX_UNISON_VOICES);
        self.common.config.lock().unison = self.common.unison;
    }

    set_param_method!(set_level, level, level.clamp(0.0, 1.0));
    set_param_method!(
        set_pitch_shift,
        pitch_shift,
        pitch_shift.clamp(st_to_octave(-60.0), st_to_octave(60.0))
    );
    set_param_method!(set_detune, detune, detune.clamp(0.0, st_to_octave(1.0)));
    set_param_method!(set_phase_shift, phase_shift, phase_shift.clamp(-1.0, 1.0));

    pub fn set_initial_phase(&mut self, voice_idx: usize, phase: StereoSample) {
        for (channel, phase) in self.channels.iter_mut().zip(phase.iter()) {
            channel.initial_phases[voice_idx] = *phase;
        }

        let mut cfg = self.common.config.lock();

        for (channel_cfg, channel) in cfg.channels.iter_mut().zip(self.channels.iter()) {
            channel_cfg.initial_phases[voice_idx] = channel.initial_phases[voice_idx];
        }
    }

    #[inline(always)]
    fn get_wave_slice_mut(wave_buff: &mut WaveformBuffer) -> &mut [Sample] {
        &mut wave_buff[WAVEFORM_PAD_LEFT..(WAVEFORM_BUFFER_SIZE - WAVEFORM_PAD_RIGHT)]
    }

    #[inline(always)]
    fn get_interpolated_sample(wave_buff: &WaveformBuffer, idx: usize, t: Sample) -> Sample {
        spline_segment::<CatmullRom, _, _>(
            t,
            &wave_buff[idx..(idx + WAVEFORM_PAD_LEFT + WAVEFORM_PAD_RIGHT + 1)],
        )
    }

    #[inline(always)]
    fn wrap_wave_buffer(wave_buff: &mut WaveformBuffer) {
        wave_buff[0] = wave_buff[WAVEFORM_BUFFER_SIZE - WAVEFORM_PAD_RIGHT - 1];
        wave_buff[WAVEFORM_BUFFER_SIZE - WAVEFORM_PAD_RIGHT] = wave_buff[WAVEFORM_PAD_LEFT];
        wave_buff[WAVEFORM_BUFFER_SIZE - WAVEFORM_PAD_RIGHT + 1] = wave_buff[WAVEFORM_PAD_LEFT + 1];
    }

    #[inline(always)]
    fn to_int_phase(phase: Sample) -> Phase {
        (phase * FULL_PHASE) as i64 as Phase
    }

    fn build_wave(
        inverse_fft: &dyn ComplexToReal<Sample>,
        frequency: f32,
        sample_rate: f32,
        spectral_buff: &SpectralBuffer,
        tmp_spectral_buff: &mut SpectralBuffer,
        scratch_buff: &mut SpectralBuffer,
        out_wave_buff: &mut WaveformBuffer,
    ) {
        let cutoff_index =
            ((0.5 * sample_rate / frequency).floor() as usize + 1).min(spectral_buff.len() - 1);

        *tmp_spectral_buff = *spectral_buff;
        tmp_spectral_buff[cutoff_index..].fill(ComplexSample::ZERO);

        inverse_fft
            .process_with_scratch(
                tmp_spectral_buff,
                Self::get_wave_slice_mut(out_wave_buff),
                scratch_buff,
            )
            .unwrap();
        Self::wrap_wave_buffer(out_wave_buff);
    }

    #[inline(always)]
    // #[unsafe(no_mangle)]
    fn process_sample(
        octave: Sample,
        phase_shift: Phase,
        buff_t: Sample,
        wave_from: &WaveformBuffer,
        wave_to: &WaveformBuffer,
        freq_phase_mult: Sample,
        phase: &mut Phase,
    ) -> Sample {
        let shifted_phase = phase.wrapping_add(phase_shift);
        let idx = (shifted_phase >> INTERMEDIATE_BITS) as usize;
        let t = (shifted_phase & INTERMEDIATE_MASK) as Sample * INTERMEDIATE_MULT;
        let sample_from = Self::get_interpolated_sample(wave_from, idx, t);
        let sample_to = Self::get_interpolated_sample(wave_to, idx, t);
        let result = sample_from + (sample_to - sample_from) * buff_t;

        *phase = phase.wrapping_add((octave_to_freq(octave) * freq_phase_mult) as i64 as u32);
        result
    }

    fn process_channel_voice(
        common: &mut Common,
        channel: &mut Channel,
        params: &ProcessParams,
        router: &dyn Router,
        voice_idx: usize,
        channel_idx: usize,
    ) {
        let id = common.id;
        let sample_rate = params.sample_rate;
        let voice = &mut channel.voices[voice_idx];

        let level_mod = router
            .get_input(
                ModuleInput::level(id),
                params.samples,
                voice_idx,
                channel_idx,
                &mut common.level_mod_input,
            )
            .unwrap_or(&ONES_BUFFER);

        let pitch_shift_mod = router
            .get_input(
                ModuleInput::pitch_shift(id),
                params.samples,
                voice_idx,
                channel_idx,
                &mut common.pitch_shift_input,
            )
            .unwrap_or(&ZEROES_BUFFER);

        let phase_shift_mod = router
            .get_input(
                ModuleInput::phase_shift(id),
                params.samples,
                voice_idx,
                channel_idx,
                &mut common.phase_shift_input,
            )
            .unwrap_or(&ZEROES_BUFFER);

        let spectrum = router
            .get_spectral_input(ModuleInput::spectrum(id), voice_idx, channel_idx)
            .unwrap_or(&ZEROES_SPECTRAL_BUFFER);

        let (wave_from, wave_to) = if voice.wave_buffers_swapped {
            (&voice.wave_buffers.1, &mut voice.wave_buffers.0)
        } else {
            (&voice.wave_buffers.0, &mut voice.wave_buffers.1)
        };

        Self::build_wave(
            common.inverse_fft.as_ref(),
            octave_to_freq(voice.octave + channel.pitch_shift + pitch_shift_mod[0]),
            params.sample_rate,
            spectrum,
            &mut common.tmp_spectral_buff,
            &mut common.scratch_buff,
            wave_to,
        );
        voice.wave_buffers_swapped = !voice.wave_buffers_swapped;

        let freq_phase_mult = FULL_PHASE / sample_rate;
        let buff_t_mult = (params.samples as f32).recip();
        let fixed_octave = voice.octave + channel.pitch_shift;

        if common.unison > 1 {
            let detune_mod = router
                .get_input(
                    ModuleInput::detune(id),
                    params.samples,
                    voice_idx,
                    channel_idx,
                    &mut common.detune_mod_input,
                )
                .unwrap_or(&ZEROES_BUFFER);

            let unison_mult = ((common.unison - 1) as Sample).recip();
            let unison_scale = 1.0 / (common.unison as Sample).sqrt();

            for (out, level_mod, pitch_shift_mod, phase_shift_mod, detune_mod, sample_idx) in izip!(
                &mut voice.output,
                level_mod,
                pitch_shift_mod,
                phase_shift_mod,
                detune_mod,
                0..params.samples
            ) {
                let mut sample: Sample = 0.0;
                let buff_t = sample_idx as Sample * buff_t_mult;
                let octave = fixed_octave + *pitch_shift_mod;
                let detune = channel.detune + *detune_mod;
                let unison_pitch_step = detune * unison_mult;
                let unison_pitch_from = -0.5 * detune;
                let phase_shift = Self::to_int_phase(channel.phase_shift + *phase_shift_mod);

                for unison_idx in 0..common.unison {
                    let unison_idx_float = unison_idx as Sample;
                    let unison_pitch_shift =
                        unison_pitch_from + unison_pitch_step * unison_idx_float;
                    let phase = &mut voice.phases[unison_idx];

                    sample += Self::process_sample(
                        octave + unison_pitch_shift,
                        phase_shift,
                        buff_t,
                        wave_from,
                        wave_to,
                        freq_phase_mult,
                        phase,
                    );
                }

                *out = sample * unison_scale * channel.level * level_mod;
            }
        } else {
            let phase = &mut voice.phases[0];

            for (out, level_mod, pitch_shift_mod, phase_shift_mod, sample_idx) in izip!(
                &mut voice.output,
                level_mod,
                pitch_shift_mod,
                phase_shift_mod,
                0..params.samples
            ) {
                *out = Self::process_sample(
                    fixed_octave + *pitch_shift_mod,
                    Self::to_int_phase(channel.phase_shift + *phase_shift_mod),
                    sample_idx as Sample * buff_t_mult,
                    wave_from,
                    wave_to,
                    freq_phase_mult,
                    phase,
                ) * channel.level
                    * level_mod;
            }
        }
    }
}

impl SynthModule for Oscillator {
    fn id(&self) -> ModuleId {
        self.common.id
    }

    fn label(&self) -> String {
        self.common.label.clone()
    }

    fn set_label(&mut self, label: String) {
        self.common.label = label.clone();
        self.common.config.lock().label = Some(label);
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::Oscillator
    }

    fn is_spectral_rate(&self) -> bool {
        false
    }

    fn inputs(&self) -> &'static [InputType] {
        &[
            InputType::Level,
            InputType::PitchShift,
            InputType::PhaseShift,
            InputType::Detune,
            InputType::Spectrum,
        ]
    }

    fn output_type(&self) -> OutputType {
        OutputType::Audio
    }

    fn note_on(&mut self, params: &NoteOnParams, router: &dyn Router) {
        for (channel_idx, channel) in self.channels.iter_mut().enumerate() {
            let voice = &mut channel.voices[params.voice_idx];

            voice.octave = note_to_octave(params.note);

            if params.reset {
                for (phase, initial_phase) in voice.phases.iter_mut().zip(channel.initial_phases) {
                    *phase = Self::to_int_phase(initial_phase);
                }

                let pitch_shift_mod = router
                    .get_input(
                        ModuleInput::pitch_shift(self.common.id),
                        1,
                        params.voice_idx,
                        channel_idx,
                        &mut self.common.pitch_shift_input,
                    )
                    .unwrap_or(&ZEROES_BUFFER);

                let spectrum = router
                    .get_spectral_input(
                        ModuleInput::spectrum(self.common.id),
                        params.voice_idx,
                        channel_idx,
                    )
                    .unwrap_or(&ZEROES_SPECTRAL_BUFFER);

                Self::build_wave(
                    self.common.inverse_fft.as_ref(),
                    octave_to_freq(voice.octave + channel.pitch_shift + pitch_shift_mod[0]),
                    params.sample_rate,
                    spectrum,
                    &mut self.common.tmp_spectral_buff,
                    &mut self.common.scratch_buff,
                    &mut voice.wave_buffers.0,
                );

                voice.wave_buffers_swapped = false;
            }
        }
    }

    fn process(&mut self, params: &ProcessParams, router: &dyn Router) {
        for (channel_idx, channel) in self.channels.iter_mut().enumerate() {
            for voice_idx in params.active_voices {
                Self::process_channel_voice(
                    &mut self.common,
                    channel,
                    params,
                    router,
                    *voice_idx,
                    channel_idx,
                );
            }
        }
    }

    fn get_buffer_output(&self, voice_idx: usize, channel: usize) -> &Buffer {
        &self.channels[channel].voices[voice_idx].output
    }
}
