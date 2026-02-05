use std::sync::Arc;

use axum::{Json, Router, http::StatusCode, response::IntoResponse};
use parking_lot::Mutex;
use serde::Serialize;
use tokio::runtime::Runtime;

use crate::{
    api::{
        engine::engine_router, oscillator::oscillator_router,
        spectral_filter::spectral_filter_router,
    },
    synth_engine::{ModuleId, SynthEngine},
};

mod engine;
mod oscillator;
mod spectral_filter;

pub type SynthRef = Arc<Mutex<SynthEngine>>;

pub enum ApiError {
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

#[macro_export]
macro_rules! get_typed_module {
    ($synth:ident, $module_type:ident, $module_id:expr) => {
        $synth
            .get_module_mut($module_id)
            .and_then(|module| $module_type::downcast_mut(module))
            .ok_or(ApiError::ModuleNotFound($module_id))
    };
}

fn build_router(synth_engine: SynthRef) -> Router {
    Router::new()
        .merge(engine_router())
        .nest("/oscillator/{id}", oscillator_router())
        .nest("/spectral-filter/{id}", spectral_filter_router())
        .with_state(synth_engine)
}

pub struct ApiServer {
    runtime: Mutex<Option<Runtime>>,
    synth_engine: Arc<Mutex<SynthEngine>>,
}

impl ApiServer {
    pub fn new(synth_engine: Arc<Mutex<SynthEngine>>) -> Self {
        Self {
            runtime: Mutex::new(None),
            synth_engine,
        }
    }

    pub fn is_running(&self) -> bool {
        self.runtime.lock().is_some()
    }

    pub fn start(&self) {
        if self.runtime.lock().is_some() {
            return;
        }

        let Some(runtime) = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .ok()
        else {
            return;
        };

        let router = build_router(Arc::clone(&self.synth_engine));

        runtime.spawn(async move {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:7688").await?;

            axum::serve(listener, router).await
        });

        *self.runtime.lock() = Some(runtime);
    }

    pub fn stop(&self) {
        if let Some(runtime) = self.runtime.lock().take() {
            runtime.shutdown_background();
        }
    }
}
