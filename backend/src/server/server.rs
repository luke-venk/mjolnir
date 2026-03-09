use crate::{server::app_state::AppState, schemas::EventType};
use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use serde_json::json;

// Informs us that server is up and running.
async fn health_check() -> impl IntoResponse {
    Json(json!({
        "status": "ok",
        "message": "Server is running.",
    }))
}

// Request and response bodies for specifying the type of throwing event.
#[derive(Deserialize)]
struct PostEventTypeRequest {
    event_type: String,
}

#[derive(Serialize)]
struct GetEventTypeResponse {
    event_type: EventType,
}

// Allows manual input of what type of event the next throw will be.
async fn post_event_type(
    State(state): State<AppState>,
    Json(payload): Json<PostEventTypeRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let event_type = match payload.event_type.as_str() {
        "shotput" => EventType::Shotput,
        "discus" => EventType::Discus,
        "hammer" => EventType::Hammer,
        "javelin" => EventType::Javelin,
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                "Error: User specified invalid throwing event. Please choose one of 'shotput','discus', 'hammer', or 'javelin'.".to_string(),
            ))
        }
    };

    *state.event_type.write().await = event_type;
    Ok(StatusCode::OK)
}

// Allows query of what the type of event the current throw is.
async fn get_event_type(State(state): State<AppState>) -> Json<GetEventTypeResponse> {
    let event_type = *state.event_type.read().await;
    Json(GetEventTypeResponse { event_type })
}

pub fn create_app() -> Router {
    let state = AppState::new();

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
    use crate::server::create_app;
    use axum::{body::Body, http::Request};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_health_check() {
        let app: Router = create_app();

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

    #[tokio::test]
    async fn test_default_event_type_is_shotput() {
        let app: Router = create_app();

        let request = Request::builder()
            .uri("/event_type")
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["event_type"], "Shotput");
    }

    #[tokio::test]
    async fn test_valid_event_type_post_and_get() {
        let app: Router = create_app();

        let post_request = Request::builder()
            .method("POST")
            .uri("/event_type")
            .header("Content-Type", "application/json")
            .body(Body::from(r#"{"event_type":"discus"}"#))
            .unwrap();
        let post_response = app.clone().oneshot(post_request).await.unwrap();
        assert_eq!(post_response.status(), StatusCode::OK);

        let get_request = Request::builder()
            .uri("/event_type")
            .body(Body::empty())
            .unwrap();
        let get_response = app.oneshot(get_request).await.unwrap();
        assert_eq!(get_response.status(), StatusCode::OK);

        let body = get_response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["event_type"], "Discus");
    }

    #[tokio::test]
    async fn test_invalid_event_type_post() {
        let app: Router = create_app();

        let request = Request::builder()
            .method("POST")
            .uri("/event_type")
            .header("Content-Type", "application/json")
            .body(Body::from(r#"{"event_type":"curling"}"#))
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}
