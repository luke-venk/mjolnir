use backend_lib::schemas::CameraId;
use backend_lib::pipeline::Pipeline;
use backend_lib::server::{create_api_router, start_server};

use axum::Router;
use axum_embed::ServeEmbed;
use rust_embed::Embed;

// The env var `EMBEDDED_FRONTEND_DIR` is where Bazel placed the frontend
// static exports, so rust_embed can embed those into this binary.
#[derive(Embed, Clone)]
#[folder = "${EMBEDDED_FRONTEND_DIR}"]
pub struct Asset;

// In prod mode, the backend will serve the API but instead of serving
// the Next.js server, it will embed the frontend's static exports using
// rust-embed and serve using axum_embed.
pub fn create_prod_app() -> Router {
    let serve_assets = ServeEmbed::<Asset>::new();
        
    // Use the fallback service so any request that isn't one of the
    // API's routes will be directed to the frontend static exports.
    create_api_router()
        .fallback_service(serve_assets)
}

// Only start per-camera CV pipelines if using real data instead of
// simulating random throw data.
#[cfg(feature = "real")]
fn start_pipelines() {
    // Start the 2 pipelines (one for each camera).
    let rolling_buffer_size: usize = 10;
    let _ = Pipeline::new(CameraId::FieldLeft, rolling_buffer_size);
    let _ = Pipeline::new(CameraId::FieldRight, rolling_buffer_size);
    println!("Running prod backend with real throw data on localhost:5001/.");
}

// If simulating random throw data, no need to start pipelines.
#[cfg(feature = "fake")]
fn start_pipelines() {
    println!("Running prod backend with simulated throw data on localhost:5001/.");
}

// Start tokio async runtime.
#[tokio::main]
async fn main() {
    // Start CV pipelines if using real data, otherwise do nothing.
    start_pipelines();

    // TODO(#7): Implement Clean Shutdown.

    // Build the Axum router.
    let app = create_prod_app();

    // Start the Axum server.
    start_server(app, "0.0.0.0:5001").await;
}
