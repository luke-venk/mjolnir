use std::sync::Arc;
use axum::{Json, Router, extract::State, http::StatusCode, response::IntoResponse, routing::{get, post}};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::RwLock;
use crate::{server::app_state::AppState, sports::EventType};

// Custom error enum to centralize HTTP error mapping for Axum server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum ApiError {
    InvalidEvent,
}

// Request and response bodies for specifying the type of throwing event.
#[derive(Deserialize)]
struct PostEventTypeRequest {
    event_type: String,
}

#[derive(Serialize)]
struct GetEventTypeResponse {
    event_type: Option<EventType>,
}

// Ensure handler knows how to return custom ApiError as part of HTTP response.
impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let (status, error_message) = match self {
            ApiError::InvalidEvent => (
                StatusCode::BAD_REQUEST,
                "Error: User specified invalid throwing event. Please choose one of 'shotput','discus', 'hammer', or 'javelin'.",
            ),
        };

        let body = Json(json!({
            "error": error_message
        }));

        (status, body).into_response()
    }
}

// Informs us that server is up and running.
async fn health_check() -> impl IntoResponse {
    Json(json!({
        "status": "ok",
        "message": "Server is running.",
    }))
}

// Allows manual input of what type of event the next throw will be.
async fn post_event_type(
    State(state): State<AppState>,
    Json(payload): Json<PostEventTypeRequest>,
) -> Result<StatusCode, ApiError> {
    let event_type = match payload.event_type.as_str() {
        "shotput" => EventType::Shotput,
        "discus" => EventType::Discus,
        "hammer" => EventType::Hammer,
        "javelin" => EventType::Javelin,
        _ => return Err(ApiError::InvalidEvent),
    };

    *state.event_type.write().await = Some(event_type);
    Ok(StatusCode::OK)
}

// Allows query of what the type of event the current throw is.
async fn get_event_type(
    State(state): State<AppState>,
) -> Json<GetEventTypeResponse> {
    let event_type = *state.event_type.read().await;
    Json(GetEventTypeResponse { event_type })
}

pub fn create_app() -> Router {
    // Store current event type, initially set to none, in app state for
    // all threads to safely access.
    let state = AppState {
        event_type: Arc::new(RwLock::new(None)),
    };

    Router::new()
        .route("/health", get(health_check))
        .route("/event_type", post(post_event_type).get(get_event_type))
        .with_state(state)
}

pub async fn start_server(app: Router, addr: &str) {
    // Listener.
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind TCP listener.");

    // Start the server.
    axum::serve(listener, app)
        .await
        .expect("Failed to start server.");
}


#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request};
    use http_body_util::BodyExt;
    use tower::ServiceExt;
    use crate::server::create_app;

    #[tokio::test]
    async fn test_health_check() {
        let app = create_app();

        let request = Request::builder()
            .uri("/health")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(json["status"], "ok");
        assert_eq!(json["message"], "Server is running.");
    }
}
