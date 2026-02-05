use axum::{
    Json, Router,
    extract::{Path, State},
    routing::{get, post},
};
use serde::Deserialize;

use crate::{
    api::{ApiError, SynthRef},
    get_typed_module,
    synth_engine::{ModuleId, Oscillator, OscillatorUIData, StereoSample, SynthModule},
};

#[derive(Deserialize)]
enum OscParam {
    Label(String),
    Gain(StereoSample),
    PitchShift(StereoSample),
    PhaseShift(StereoSample),
    FrequencyShift(StereoSample),
    Detune(StereoSample),
    Unison(usize),
    ResetPhase(bool),
    InitialPhase(usize, StereoSample),
}

async fn oscillator_params(
    State(synth): State<SynthRef>,
    Path(id): Path<ModuleId>,
) -> Result<Json<OscillatorUIData>, ApiError> {
    let mut synth = synth.lock();
    let osc = get_typed_module!(synth, Oscillator, id)?;

    Ok(Json(osc.get_ui()))
}

async fn set_param(
    State(synth): State<SynthRef>,
    Path(id): Path<ModuleId>,
    Json(param): Json<OscParam>,
) -> Result<(), ApiError> {
    let mut synth = synth.lock();
    let osc = get_typed_module!(synth, Oscillator, id)?;

    match param {
        OscParam::Label(label) => osc.set_label(label),
        OscParam::Gain(gain) => osc.set_gain(gain),
        OscParam::PitchShift(pitch_shift) => osc.set_pitch_shift(pitch_shift),
        OscParam::PhaseShift(phase_shift) => osc.set_phase_shift(phase_shift),
        OscParam::FrequencyShift(freq_shift) => osc.set_frequency_shift(freq_shift),
        OscParam::Detune(detune) => osc.set_detune(detune),
        OscParam::Unison(unison) => osc.set_unison(unison),
        OscParam::ResetPhase(reset) => osc.set_reset_phase(reset),
        OscParam::InitialPhase(idx, phase) => osc.set_initial_phase(idx, phase),
    }

    Ok(())
}

pub fn oscillator_router() -> Router<SynthRef> {
    Router::new()
        .route("/", get(oscillator_params))
        .route("/param", post(set_param))
}
