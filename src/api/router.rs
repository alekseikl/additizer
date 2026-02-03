use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
};
use parking_lot::Mutex;
use serde::Serialize;

use crate::synth_engine::{
    ModuleId, ModuleType, Oscillator, OscillatorUIData, SynthEngine, SynthEngineUiData,
};

type SynthRef = Arc<Mutex<SynthEngine>>;

enum ApiError {
    ModuleNotFound(ModuleId),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct ErrorResponse {
            message: String,
        }

        let (status, message) = match &self {
            Self::ModuleNotFound(id) => {
                (StatusCode::BAD_REQUEST, format!("Module {id} not found."))
            }
        };

        (status, Json(ErrorResponse { message })).into_response()
    }
}

async fn engine_params(State(synth): State<SynthRef>) -> Json<SynthEngineUiData> {
    Json(synth.lock().get_ui())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ModuleListItem {
    id: ModuleId,
    module_type: ModuleType,
    label: String,
}

async fn modules(State(synth): State<SynthRef>) -> Json<Vec<ModuleListItem>> {
    Json(
        synth
            .lock()
            .get_modules()
            .iter()
            .map(|module| ModuleListItem {
                id: module.id(),
                module_type: module.module_type(),
                label: module.label(),
            })
            .collect(),
    )
}

macro_rules! get_typed_module {
    ($synth:ident, $module_type:ident, $module_id:expr) => {
        $synth
            .get_module_mut($module_id)
            .and_then(|module| $module_type::downcast_mut(module))
            .ok_or(ApiError::ModuleNotFound($module_id))
    };
}

async fn oscillator_params(
    State(synth): State<SynthRef>,
    Path(id): Path<ModuleId>,
) -> Result<Json<OscillatorUIData>, ApiError> {
    let mut synth = synth.lock();
    let osc = get_typed_module!(synth, Oscillator, id)?;

    Ok(Json(osc.get_ui()))
}

pub fn build_router(synth_engine: SynthRef) -> Router {
    Router::new()
        .route("/", get(engine_params))
        .route("/modules", get(modules))
        .route("/oscillator/{id}", get(oscillator_params))
        .with_state(synth_engine)
}
