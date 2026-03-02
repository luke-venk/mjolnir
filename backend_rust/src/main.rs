use crate::throwing_event::EventType;

use axum::{Json, Router, http::StatusCode, response::IntoResponse, routing::{get, post}};
use serde::{Deserialize, Serialize};
use serde_json::json;

mod camera;
mod frame;
mod pipeline;
mod queue;
mod throwing_event;

// Custom error enum to centralize HTTP error mapping for Axum server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum ApiError {
    InvalidEvent,
}

// POST request and response bodies for specifying the type of throwing event.
#[derive(Deserialize)]
struct SpecifyEventTypeRequest {
    event_type: String,
}

#[derive(Serialize)]
struct SpecifyEventTypeResponse {
    event_type: EventType,
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
async fn specify_event_type(
    Json(payload): Json<SpecifyEventTypeRequest>,
) -> Result<Json<SpecifyEventTypeResponse>, ApiError> {
    let event_type = match payload.event_type.as_str() {
        "shotput" => EventType::Shotput,
        "discus" => EventType::Shotput,
        "hammer" => EventType::Shotput,
        "javelin" => EventType::Shotput,
        _ => return Err(ApiError::InvalidEvent),
    };
    Ok(Json(SpecifyEventTypeResponse { event_type }))
}

fn create_app() -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/event_type", post(specify_event_type))
}

// Start tokio async runtime.
#[tokio::main]
async fn main() {
    // Router.
    let app = create_app();

    // Listener.
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .expect("Failed to bind TCP listener.");

    // Start the server.
    axum::serve(listener, app)
        .await
        .expect("Failed to start server.");

    println!("Server running on http://localhost:3000");
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

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
