use crate::{
    synth_engine::{
        Envelope, EnvelopeCurve, ModuleInput, OUTPUT_MODULE_ID, Oscillator, SpectralFilter,
        StereoSample, SynthEngine,
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

    let filter_env = typed_module_mut!(filter_env_id, Envelope).unwrap();

    filter_env.set_attack(0.0.into());
    filter_env.set_decay(from_ms(500.0).into());
    filter_env.set_sustain(0.0.into());
    filter_env.set_release(from_ms(100.0).into());

    typed_module_mut!(filter_env_id, Envelope)
        .unwrap()
        .set_decay_curve(EnvelopeCurve::ExponentialOut { full_range: true });

    typed_module_mut!(filter_env_id, Envelope)
        .unwrap()
        .set_attack_curve(EnvelopeCurve::ExponentialIn { full_range: true });

    typed_module_mut!(filter_id, SpectralFilter)
        .unwrap()
        .set_cutoff(2.0.into());

    let osc = typed_module_mut!(osc_id, Oscillator).unwrap();

    osc.set_unison(3);
    osc.set_detune(st_to_octave(0.01).into());

    let amp_env = typed_module_mut!(amp_env_id, Envelope).unwrap();

    amp_env.set_attack(StereoSample::splat(from_ms(10.0)));
    amp_env.set_decay(from_ms(20.0).into());
    amp_env.set_sustain(1.0.into());
    amp_env.set_release(from_ms(300.0).into());

    typed_module_mut!(amp_env_id, Envelope)
        .unwrap()
        .set_decay_curve(EnvelopeCurve::ExponentialOut { full_range: true });

    synth
        .add_link(harmonic_editor_id, ModuleInput::spectrum(filter_id))
        .unwrap();

    synth
        .add_modulation(
            filter_env_id,
            ModuleInput::cutoff(filter_id),
            st_to_octave(64.0).into(),
        )
        .unwrap();

    synth
        .add_link(filter_id, ModuleInput::spectrum(osc_id))
        .unwrap();

    synth.add_link(osc_id, ModuleInput::audio(amp_id)).unwrap();

    synth
        .add_link(amp_env_id, ModuleInput::level(amp_id))
        .unwrap();

    synth
        .add_link(amp_id, ModuleInput::audio(OUTPUT_MODULE_ID))
        .unwrap();
}
