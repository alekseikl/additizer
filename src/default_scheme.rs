use crate::{
    synth_engine::{
        Amplifier, Envelope, EnvelopeCurve, HarmonicEditor, Input, ModuleInput, OUTPUT_MODULE_ID,
        SpectralFilter, SynthEngine, SynthModule, oscillator::Oscillator,
    },
    utils::{from_ms, st_to_octave},
};

pub fn build_default_scheme(synth: &mut SynthEngine) {
    let harmonic_editor_id = synth.add_harmonic_editor();
    let filter_env_id = synth.add_envelope();
    let filter_id = synth.add_spectral_filter();
    let osc_id = synth.add_oscillator();
    let amp_id = synth.add_amplifier();
    let amp_env_id = synth.add_envelope();

    macro_rules! typed_module_mut {
        ($module_id:expr, $module_type:ident) => {
            synth
                .get_module_mut($module_id)
                .and_then(|module| $module_type::downcast_mut(module))
        };
    }

    let editor = typed_module_mut!(harmonic_editor_id, HarmonicEditor).unwrap();

    editor.set_label("01 - Harmonics".into());

    let filter_env = typed_module_mut!(filter_env_id, Envelope).unwrap();

    filter_env.set_label("03 - Cutoff Env".into());
    filter_env.set_attack(0.0.into());
    filter_env.set_decay(from_ms(500.0).into());
    filter_env.set_sustain(0.0.into());
    filter_env.set_release(from_ms(100.0).into());
    filter_env.set_decay_curve(EnvelopeCurve::ExponentialOut);
    filter_env.set_attack_curve(EnvelopeCurve::ExponentialOut);

    let spectral_filter = typed_module_mut!(filter_id, SpectralFilter).unwrap();

    spectral_filter.set_label("03 - Filter".into());
    spectral_filter.set_cutoff(2.0.into());

    let osc = typed_module_mut!(osc_id, Oscillator).unwrap();

    osc.set_label("04 - Oscillator".into());
    osc.set_unison(1);
    osc.set_detune(st_to_octave(0.1).into());

    let amp_env = typed_module_mut!(amp_env_id, Envelope).unwrap();

    amp_env.set_label("06 - Amp Envelope".into());
    amp_env.set_decay(from_ms(400.0).into());
    amp_env.set_sustain(0.6.into());
    amp_env.set_release(from_ms(300.0).into());
    amp_env.set_decay_curve(EnvelopeCurve::ExponentialOut);
    amp_env.set_smooth(from_ms(4.0).into());
    amp_env.set_keep_voice_alive(true);

    let amp = typed_module_mut!(amp_id, Amplifier).unwrap();

    amp.set_label("06 - Amplifier".into());

    synth
        .set_direct_link(
            harmonic_editor_id,
            ModuleInput::new(Input::Spectrum, filter_id),
        )
        .unwrap();

    synth
        .add_link(
            filter_env_id,
            ModuleInput::new(Input::Cutoff, filter_id),
            st_to_octave(64.0).into(),
        )
        .unwrap();

    synth
        .set_direct_link(filter_id, ModuleInput::new(Input::Spectrum, osc_id))
        .unwrap();

    synth
        .set_direct_link(osc_id, ModuleInput::new(Input::Audio, amp_id))
        .unwrap();

    synth
        .set_direct_link(amp_env_id, ModuleInput::new(Input::Gain, amp_id))
        .unwrap();

    synth
        .set_direct_link(amp_id, ModuleInput::new(Input::Audio, OUTPUT_MODULE_ID))
        .unwrap();
}
