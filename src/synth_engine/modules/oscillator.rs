use std::{any::Any, f32, sync::Arc};

use itertools::izip;
use rand::Rng;
use rand_pcg::Pcg32;
use realfft::{ComplexToReal, RealFftPlanner};
use serde::{Deserialize, Serialize};

use crate::{
    synth_engine::{
        buffer::{
            Buffer, ONES_BUFFER, SpectralBuffer, WAVEFORM_BITS, WAVEFORM_SIZE, WaveformBuffer,
            ZEROES_BUFFER, ZEROES_SPECTRAL_BUFFER, get_interpolated_sample, get_wave_slice_mut,
            make_zero_buffer, make_zero_spectral_buffer, make_zero_wave_buffer, wrap_wave_buffer,
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

const FULL_PHASE: f32 = ((u32::MAX as u64) + 1) as f32;
const INTERMEDIATE_BITS: usize = 32 - WAVEFORM_BITS;
const INTERMEDIATE_MASK: u32 = (1 << INTERMEDIATE_BITS) - 1;
const INTERMEDIATE_MULT: f32 = ((1 << INTERMEDIATE_BITS) as f32).recip();
const MAX_UNISON_VOICES: usize = 16;

#[derive(Clone, Serialize, Deserialize)]
pub struct OscillatorConfigChannel {
    level: f32,
    pitch_shift: f32,
    detune: f32,
}

impl Default for OscillatorConfigChannel {
    fn default() -> Self {
        Self {
            level: 1.0,
            pitch_shift: 0.0,
            detune: st_to_octave(0.2),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct OscillatorConfig {
    label: Option<String>,
    unison: usize,
    same_channel_phases: bool,
    channels: [OscillatorConfigChannel; NUM_CHANNELS],
}

impl Default for OscillatorConfig {
    fn default() -> Self {
        Self {
            label: None,
            unison: 1,
            same_channel_phases: false,
            channels: Default::default(),
        }
    }
}

pub struct OscillatorUIData {
    pub label: String,
    pub level: StereoSample,
    pub pitch_shift: StereoSample,
    pub detune: StereoSample,
    pub unison: usize,
    pub same_channel_phases: bool,
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
    voices: [Voice; MAX_VOICES],
}

impl Default for Channel {
    fn default() -> Self {
        Self {
            level: 0.5,
            pitch_shift: 0.0,
            detune: st_to_octave(0.3),
            voices: Default::default(),
        }
    }
}

struct Common {
    id: ModuleId,
    label: String,
    config: ModuleConfigBox<OscillatorConfig>,
    unison: usize,
    same_channel_phases: bool,
    random: Pcg32,
    inverse_fft: Arc<dyn ComplexToReal<Sample>>,
    tmp_spectral_buff: SpectralBuffer,
    scratch_buff: SpectralBuffer,
    level_mod_input: Buffer,
    pitch_shift_input: Buffer,
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
                same_channel_phases: false,
                random: Pcg32::new(3537, 9573),
                inverse_fft: RealFftPlanner::<Sample>::new().plan_fft_inverse(WAVEFORM_SIZE),
                tmp_spectral_buff: make_zero_spectral_buffer(),
                scratch_buff: make_zero_spectral_buffer(),
                level_mod_input: make_zero_buffer(),
                pitch_shift_input: make_zero_buffer(),
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
                channel.detune = cfg_channel.detune;
            }
            osc.common.unison = cfg.unison;
            osc.common.same_channel_phases = cfg.same_channel_phases;
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
            unison: self.common.unison,
            same_channel_phases: self.common.same_channel_phases,
        }
    }

    pub fn set_unison(&mut self, unison: usize) {
        self.common.unison = unison.clamp(1, MAX_UNISON_VOICES);

        {
            let mut cfg = self.common.config.lock();
            cfg.unison = self.common.unison;
        }
    }

    pub fn set_same_channels_phases(&mut self, same: bool) {
        self.common.same_channel_phases = same;
        self.common.config.lock().same_channel_phases = same;
    }

    set_param_method!(set_level, level, level.clamp(0.0, 1.0));
    set_param_method!(
        set_pitch_shift,
        pitch_shift,
        pitch_shift.clamp(st_to_octave(-60.0), st_to_octave(60.0))
    );
    set_param_method!(set_detune, detune, detune.clamp(0.0, st_to_octave(1.0)));

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
                get_wave_slice_mut(out_wave_buff),
                scratch_buff,
            )
            .unwrap();
        wrap_wave_buffer(out_wave_buff);
    }

    #[inline(always)]
    fn process_sample(
        octave: f32,
        buff_t: f32,
        wave_from: &WaveformBuffer,
        wave_to: &WaveformBuffer,
        freq_phase_mult: f32,
        phase: &mut Phase,
    ) -> Sample {
        let frequency = octave_to_freq(octave);
        let idx = (*phase >> INTERMEDIATE_BITS) as usize;
        let t = (*phase & INTERMEDIATE_MASK) as f32 * INTERMEDIATE_MULT;
        let sample_from = get_interpolated_sample(wave_from, idx, t);
        let sample_to = get_interpolated_sample(wave_to, idx, t);

        *phase = phase.wrapping_add((frequency * freq_phase_mult) as u32);
        sample_from * (1.0 - buff_t) + sample_to * buff_t
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

            for (out, level_mod, pitch_shift_mod, detune_mod, sample_idx) in izip!(
                &mut voice.output,
                level_mod,
                pitch_shift_mod,
                detune_mod,
                0..params.samples
            ) {
                let mut sample: Sample = 0.0;
                let buff_t = sample_idx as f32 * buff_t_mult;
                let octave = fixed_octave + *pitch_shift_mod;
                let detune = channel.detune + *detune_mod;
                let unison_pitch_step = detune * unison_mult;
                let unison_pitch_from = -0.5 * detune;

                for unison_idx in 0..common.unison {
                    let unison_idx_float = unison_idx as f32;
                    let unison_pitch_shift =
                        unison_pitch_from + unison_pitch_step * unison_idx_float;
                    let phase = &mut voice.phases[unison_idx];

                    sample += Self::process_sample(
                        octave + unison_pitch_shift,
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

            for (out, level_mod, pitch_shift_mod, sample_idx) in izip!(
                &mut voice.output,
                level_mod,
                pitch_shift_mod,
                0..params.samples
            ) {
                *out = Self::process_sample(
                    fixed_octave + *pitch_shift_mod,
                    sample_idx as f32 * buff_t_mult,
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
            InputType::Detune,
            InputType::Spectrum,
        ]
    }

    fn output_type(&self) -> OutputType {
        OutputType::Audio
    }

    fn note_on(&mut self, params: &NoteOnParams, router: &dyn Router) {
        let trigger = !params.same_note_retrigger;

        for (channel_idx, channel) in self.channels.iter_mut().enumerate() {
            let voice = &mut channel.voices[params.voice_idx];

            voice.octave = note_to_octave(params.note);

            if trigger {
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

        if trigger {
            if self.common.same_channel_phases {
                let mut phases: [Phase; MAX_UNISON_VOICES] = [0; MAX_UNISON_VOICES];

                self.common.random.fill(&mut phases);

                for channel in &mut self.channels {
                    channel.voices[params.voice_idx]
                        .phases
                        .copy_from_slice(&phases);
                }
            } else {
                for channel in &mut self.channels {
                    self.common
                        .random
                        .fill(&mut channel.voices[params.voice_idx].phases);
                }
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
