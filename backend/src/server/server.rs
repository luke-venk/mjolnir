use crate::{schemas::ThrowType, server::app_state::AppState};
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
struct PostThrowTypeRequest {
    // Allow camelCase in frontend and snake_case in backend.
    #[serde(alias = "throwType")]
    throw_type: String,
}

#[derive(Serialize)]
struct GetThrowTypeResponse {
    throw_type: ThrowType,
}

// Allows user input of what type of event the next throw will be.
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
                "Error: User specified invalid throwing event. Please choose one of 'shotput','discus', 'hammer', or 'javelin'.".to_string(),
            ))
        }
    };

    *state.throw_type.write().await = throw_type;
    Ok(StatusCode::OK)
}

// Allows query of what the type of event the current throw is.
async fn get_throw_type(State(state): State<AppState>) -> Json<GetThrowTypeResponse> {
    let throw_type = *state.throw_type.read().await;
    Json(GetThrowTypeResponse { throw_type })
}

// In both dev and prod mode, the router will require the HTTP routes
// and the thread-safe shared app state.
pub fn create_api_router() -> Router {
    // Thread-safe shared app state for current throw event.
    let state = AppState::new();

    // Define HTTP routes.
    let http_routes = Router::new()
        .route("/health", get(health_check))
        .route("/throw-type", post(post_throw_type).get(get_throw_type));

    // Nest the routes behind the "/api" prefix so no naming collisions
    // with frontend requests.
    Router::new()
        .nest("/api", http_routes)
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

    #[tokio::test]
    async fn test_health_check() {
        let app: Router = create_api_router();

        let request = Request::builder()
            .uri("/api/health")
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
    async fn test_default_throw_type_is_shotput() {
        let app: Router = create_api_router();

        let request = Request::builder()
            .uri("/api/throw-type")
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["throw_type"], "Shotput");
    }

    #[tokio::test]
    async fn test_valid_throw_type_post_and_get() {
        let app: Router = create_api_router();

        let post_request = Request::builder()
            .method("POST")
            .uri("/api/throw-type")
            .header("Content-Type", "application/json")
            .body(Body::from(r#"{"throw_type":"discus"}"#))
            .unwrap();
        let post_response = app.clone().oneshot(post_request).await.unwrap();
        assert_eq!(post_response.status(), StatusCode::OK);

        let get_request = Request::builder()
            .uri("/api/throw-type")
            .body(Body::empty())
            .unwrap();
        let get_response = app.oneshot(get_request).await.unwrap();
        assert_eq!(get_response.status(), StatusCode::OK);

        let body = get_response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["throw_type"], "Discus");
    }

    #[tokio::test]
    async fn test_invalid_throw_type_post() {
        let app: Router = create_api_router();

        let request = Request::builder()
            .method("POST")
            .uri("/api/throw-type")
            .header("Content-Type", "application/json")
            .body(Body::from(r#"{"throw_type":"curling"}"#))
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}
