use backend_lib::schemas::CameraId;
use backend_lib::camera_ingest::start_default_camera_pipeline;
use backend_lib::server::{create_api_router, start_server};

use axum::{Router, http::Method};
use tower_http::cors::{Any, CorsLayer};

// In dev mode, the backend can serve the API via command line, and it will
// also serve the Next.js server, so it will need CORS. It will not have any
// embedded assets.
pub fn create_dev_app() -> Router {
    // Set up CORS layer to allow cross-origin sharing for integration mode.
    // Next.js requests will come from port 3000.
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::HEAD])
        .allow_headers(Any);

    create_api_router().layer(cors)
}

// Start tokio async runtime.
#[tokio::main]
async fn main() {
    // Start the 2 pipelines (one for each camera).
    let rolling_buffer_size: usize = 10;
    let _ = start_default_camera_pipeline(CameraId::FieldLeft, rolling_buffer_size);
    let _ = start_default_camera_pipeline(CameraId::FieldRight, rolling_buffer_size);

    // TODO(#7): Implement Clean Shutdown.

    // Build the Axum router.
    let app = create_dev_app();

    // Start the Axum server.
    start_server(app, "0.0.0.0:5001").await;
}
