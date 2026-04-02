use backend_lib::schemas::CameraId;
use backend_lib::pipeline::Pipeline;
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

// Only start per-camera CV pipelines if using real data instead of
// simulating random throw data.
#[cfg(feature = "real")]
fn start_pipelines() {
    // Start the 2 pipelines (one for each camera).
    let rolling_buffer_size: usize = 10;
    let _ = Pipeline::new(CameraId::FieldLeft, rolling_buffer_size);
    let _ = Pipeline::new(CameraId::FieldRight, rolling_buffer_size);
    println!("Running dev backend with real throw data on localhost:5001/.");
}

// If simulating random throw data, no need to start pipelines.
#[cfg(feature = "fake")]
fn start_pipelines() {
    println!("Running dev backend with simulated throw data on localhost:5001/.");
}

// Start tokio async runtime.
#[tokio::main]
async fn main() {
    // Start CV pipelines if using real data, otherwise do nothing.
    start_pipelines();

    // TODO(#7): Implement Clean Shutdown.

    // Build the Axum router.
    let app = create_dev_app();

    // Start the Axum server.
    start_server(app, "0.0.0.0:5001").await;
}
