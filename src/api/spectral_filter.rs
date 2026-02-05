use axum::{
    Json, Router,
    extract::{Path, State},
    routing::{get, post},
};

use crate::{
    api::{ApiError, SynthRef},
    get_typed_module,
    synth_engine::{
        ModuleId, SpectralFilter, SpectralFilterType, SpectralFilterUIData, StereoSample,
        SynthModule,
    },
    utils::st_to_octave,
};

async fn filter_params(
    State(synth): State<SynthRef>,
    Path(id): Path<ModuleId>,
) -> Result<Json<SpectralFilterUIData>, ApiError> {
    let mut synth = synth.lock();
    let filter = get_typed_module!(synth, SpectralFilter, id)?;

    Ok(Json(filter.get_ui()))
}

async fn set_label(
    State(synth): State<SynthRef>,
    Path(id): Path<ModuleId>,
    Json(label): Json<String>,
) -> Result<(), ApiError> {
    let mut synth = synth.lock();
    let filter = get_typed_module!(synth, SpectralFilter, id)?;

    filter.set_label(label);
    Ok(())
}

async fn set_type(
    State(synth): State<SynthRef>,
    Path(id): Path<ModuleId>,
    Json(filter_type): Json<SpectralFilterType>,
) -> Result<(), ApiError> {
    let mut synth = synth.lock();
    let filter = get_typed_module!(synth, SpectralFilter, id)?;

    filter.set_filter_type(filter_type);

    Ok(())
}

async fn set_cutoff(
    State(synth): State<SynthRef>,
    Path(id): Path<ModuleId>,
    Json(cutoff): Json<StereoSample>,
) -> Result<(), ApiError> {
    let mut synth = synth.lock();
    let filter = get_typed_module!(synth, SpectralFilter, id)?;

    filter.set_cutoff(cutoff.iter().map(|val| st_to_octave(*val)).collect());

    Ok(())
}

async fn set_drive(
    State(synth): State<SynthRef>,
    Path(id): Path<ModuleId>,
    Json(drive): Json<StereoSample>,
) -> Result<(), ApiError> {
    let mut synth = synth.lock();
    let filter = get_typed_module!(synth, SpectralFilter, id)?;

    filter.set_drive(drive);

    Ok(())
}

pub fn spectral_filter_router() -> Router<SynthRef> {
    Router::new()
        .route("/", get(filter_params))
        .route("/label", post(set_label))
        .route("/type", post(set_type))
        .route("/cutoff", post(set_cutoff))
        .route("/drive", post(set_drive))
}
