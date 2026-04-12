use axum::{Router, http::Method};
use backend_lib::server::{ThrowSource, create_api_router, start_server};
use tower_http::cors::{Any, CorsLayer};

#[cfg(feature = "real")]
use backend_lib::pipeline::Pipeline;
#[cfg(feature = "real")]
use backend_lib::schemas::CameraId;

// In dev mode, the backend can serve the API via command line, and it will
// also serve the Next.js server, so it will need CORS. It will not have any
// embedded assets.
pub fn create_dev_app(throw_source: ThrowSource) -> Router {
    // Set up CORS layer to allow cross-origin sharing for integration mode.
    // Next.js requests will come from port 3000.
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::HEAD])
        .allow_headers(Any);

    create_api_router(throw_source).layer(cors)
}

// The "fake" configuration will not start the CV pipelines, and will point the
// `analyze-throw` route to simulated throw data.
#[cfg(feature = "fake")]
#[tokio::main]
async fn main() {
    // Build the Axum router.
    let app = create_dev_app(ThrowSource::Simulated);

    // Start the Axum server.
    start_server(app, "0.0.0.0:5001").await;
}

// The "real" configuration will start the CV pipelines, and will point the
// `analyze-throw` route to the processed throw data from the pipelines.
#[cfg(feature = "real")]
#[tokio::main]
async fn main() {
    // Start the 2 computer vision pipelines (one for each camera).
    let rolling_buffer_size: usize = 10;
    let _ = Pipeline::new(CameraId::FieldLeft, rolling_buffer_size);
    let _ = Pipeline::new(CameraId::FieldRight, rolling_buffer_size);

    // Build the Axum router.
    let app = create_dev_app(ThrowSource::Camera);

    // Start the Axum server.
    start_server(app, "0.0.0.0:5001").await;

    // TODO(#7): Implement Clean Shutdown.
}
