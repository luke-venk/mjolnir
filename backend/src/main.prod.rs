use backend_lib::server::{ThrowSource, create_api_router, start_server};
use axum::Router;
use axum_embed::ServeEmbed;
use backend_lib::circle_infractions_ingest::begin_detecting_circle_infractions;
use rust_embed::Embed;

const ARDUINO_BAUD_RATE: u32 = 115200;
#[cfg(feature = "real_cameras")]
use backend_lib::pipeline::Pipeline;
#[cfg(feature = "real_cameras")]
use backend_lib::schemas::CameraId;

// The env var `EMBEDDED_FRONTEND_DIR` is where Bazel placed the frontend
// static exports, so rust_embed can embed those into this binary.
#[derive(Embed, Clone)]
#[folder = "${EMBEDDED_FRONTEND_DIR}"]
pub struct Asset;

// In prod mode, the backend will serve the API but instead of serving
// the Next.js server, it will embed the frontend's static exports using
// rust-embed and serve using axum_embed.
pub fn create_prod_app(throw_source: ThrowSource) -> Router {
    let serve_assets = ServeEmbed::<Asset>::new();

    let infractions_rx = begin_detecting_circle_infractions(ARDUINO_BAUD_RATE);

    // Use the fallback service so any request that isn't one of the
    // API's routes will be directed to the frontend static exports.
    create_api_router(throw_source, infractions_rx)
        .fallback_service(serve_assets)
}

// Lacking a "real_cameras" feature flag will not start the CV pipelines, and will point the
// `analyze-throw` route to simulated throw data.
#[cfg(not(feature = "real_cameras"))]
#[tokio::main]
async fn main() {
    // Build the Axum router.
    let app = create_prod_app(ThrowSource::Simulated);

    // Start the Axum server.
    start_server(app, "0.0.0.0:5001").await;
}

// The "real_cameras" configuration will start the CV pipelines, and will point the
// `analyze-throw` route to the processed throw data from the pipelines.
#[cfg(feature = "real_cameras")]
#[tokio::main]
async fn main() {
    // Start the 2 computer vision pipelines (one for each camera).
    let rolling_buffer_size: usize = 10;
    let _ = Pipeline::new(CameraId::FieldLeft, rolling_buffer_size);
    let _ = Pipeline::new(CameraId::FieldRight, rolling_buffer_size);

    // Build the Axum router.
    let app = create_prod_app(ThrowSource::Camera);
    
    // Start the Axum server.
    start_server(app, "0.0.0.0:5001").await;

    // TODO(#7): Implement Clean Shutdown.
}
