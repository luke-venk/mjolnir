use axum::{Router, http::Method};
use backend_lib::circle_infractions_ingest::begin_detecting_circle_infractions;
use backend_lib::pipeline::Pipeline;
use backend_lib::schemas::CameraId;
use backend_lib::server::{create_api_router, start_server};
use tower_http::cors::{Any, CorsLayer};

const ARDUINO_BAUD_RATE: u32 = 115200;

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

    let infractions_rx = begin_detecting_circle_infractions(ARDUINO_BAUD_RATE);

    create_api_router(infractions_rx).layer(cors)
}

// Start tokio async runtime.
#[tokio::main]
async fn main() {
    // Start the 2 pipelines (one for each camera).
    let rolling_buffer_size: usize = 10;
    let _ = Pipeline::new(CameraId::FieldLeft, rolling_buffer_size);
    let _ = Pipeline::new(CameraId::FieldRight, rolling_buffer_size);

    // TODO(#7): Implement Clean Shutdown.

    // Build the Axum router.
    let app = create_dev_app();

    // Start the Axum server.
    start_server(app, "0.0.0.0:5001").await;
}
