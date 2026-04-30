use crate::circle_infractions_ingest::CircleInfractionDetectionState;
use crate::server::app_state::AppState;
use crate::throws::ThrowType;
use crate::throws::{simulate_throw::simulate_throw_event, *};
use super::ThrowSource;

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use crossbeam::channel::Receiver;
use serde_json::json;

async fn health_check() -> impl IntoResponse {
    Json(json!({
        "status": "ok",
        "message": "Server is running.",
    }))
}

async fn post_throw_type(
    State(state): State<AppState>,
    Json(payload): Json<PostThrowTypeRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let throw_type = match payload.throw_type.as_str() {
        "shotput" => ThrowType::Shotput,
        "discus" => ThrowType::Discus,
        "hammer" => ThrowType::Hammer,
        "javelin" => ThrowType::Javelin,
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                "Error: invalid throwing event. Choose 'shotput','discus','hammer','javelin'."
                    .to_string(),
            ))
        }
    };

    *state.throw_type.write().await = throw_type;
    Ok(StatusCode::OK)
}

async fn get_throw_type(State(state): State<AppState>) -> Json<GetThrowTypeResponse> {
    let throw_type = *state.throw_type.read().await;
    Json(GetThrowTypeResponse { throw_type })
}

// history_ms contains timestamps (ms since unix epoch) when INFRACTION bytes were received.
// We add Circle iff there exists an infraction in [throw_start_ms - 2000, throw_end_ms].
fn has_circle_infraction_in_window(history_ms: &[u64], throw_start_ms: u64, throw_end_ms: u64) -> bool {
    let start = throw_start_ms.saturating_sub(2_000);
    let end = throw_end_ms;
    history_ms.iter().any(|&t| t >= start && t <= end)
}

async fn get_throw_results(State(state): State<AppState>) -> Json<ThrowAnalysisResponse> {
    let throw_type: ThrowType = *state.throw_type.read().await;

    // 1) Base throw analysis result (simulated for now)
    let mut result: ThrowAnalysisResponse = match state.throw_source {
        ThrowSource::Simulated => simulate_throw_event(throw_type),
        ThrowSource::Camera => {
            // TODO: replace with real CV pipeline output
            simulate_throw_event(throw_type)
        }
    };

    // 2) Remove any simulated Circle infractions.
    result
        .infractions
        .retain(|inf| inf.infraction_type != InfractionType::Circle);

    // 3) Compute throw window (ms) from response start/end.
    //
    // Ideal path: these should be derived from the chosen trajectory in the 60s ringbuffer.
    // Right now simulate_throw_event populates them, so we can already test the logic end-to-end.
    let start_us = result
        .throw_start_timestamp_from_camera_microseconds
        .as_ref()
        .and_then(|s| s.parse::<u64>().ok());

    let end_us = result
        .throw_end_timestamp_from_camera_microseconds
        .as_ref()
        .and_then(|s| s.parse::<u64>().ok());

    // If timestamps are missing, we cannot correlate reliably; return without Circle.
    let (Some(start_us), Some(end_us)) = (start_us, end_us) else {
        return Json(result);
    };

    let throw_start_ms = start_us / 1_000;
    let throw_end_ms = end_us / 1_000;

    // 4) Correlate with circle infraction history
    let history = state.get_infraction_history().await;
    let circle = has_circle_infraction_in_window(&history, throw_start_ms, throw_end_ms);

    if circle {
        result.infractions.push(Infraction {
            infraction_type: InfractionType::Circle,
            confidence: 1.0,
        });
    }

    Json(result)
}

async fn get_circle_status(State(state): State<AppState>) -> Json<serde_json::Value> {
    let stale = state.is_circle_infraction_system_stale().await;
    let history = state.get_infraction_history().await;
    let last_infraction_ms = history.last().copied();

    Json(json!({
        "stale": stale,
        "last_infraction_ms": last_infraction_ms,
        "history_len": history.len(),
    }))
}

pub fn create_api_router(
    throw_source: ThrowSource,
    circle_rx: Receiver<CircleInfractionDetectionState>,
) -> Router {
    let state = AppState::new(throw_source);
    let state_clone = state.clone();

    tokio::spawn(async move {
        loop {
            let state = state_clone.clone();
            let recv_result = tokio::task::spawn_blocking({
                let rx = circle_rx.clone();
                move || rx.recv()
            })
            .await;

            match recv_result {
                Ok(Ok(CircleInfractionDetectionState::DetectedInfraction(ts))) => {
                    state.set_circle_infraction_system_is_stale(false).await;

                    // Prefer approx PTP if available, else local arrival time.
                    let best_ns = ts.approx_ptp_ns.unwrap_or(ts.local_arrival_ns);
                    let ts_ms = (best_ns / 1_000_000) as u64;

                    state.record_infraction(ts_ms).await;
                }
                Ok(Ok(CircleInfractionDetectionState::KeepAlive)) => {
                    state.set_circle_infraction_system_is_stale(false).await;
                }
                Ok(Ok(CircleInfractionDetectionState::Stale)) => {
                    state.set_circle_infraction_system_is_stale(true).await;
                }
                Ok(Err(_)) | Err(_) => break,
            }
        }
    });

    let http_routes = Router::new()
        .route("/health", get(health_check))
        .route("/throw-type", post(post_throw_type).get(get_throw_type))
        .route("/analyze-throw", get(get_throw_results))
        .route("/circle-status", get(get_circle_status));

    Router::new().nest("/api", http_routes).with_state(state)
}

pub async fn start_server(app: Router, addr: &str) {
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind TCP listener.");

    axum::serve(listener, app)
        .await
        .expect("Failed to start server.");
}
