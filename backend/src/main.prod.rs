use backend_lib::server::{ThrowSource, create_api_router, start_server};
use axum::Router;
use axum_embed::ServeEmbed;
use rust_embed::Embed;

#[cfg(feature = "real")]
use backend_lib::pipeline::Pipeline;
#[cfg(feature = "real")]
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
        
    // Use the fallback service so any request that isn't one of the
    // API's routes will be directed to the frontend static exports.
    create_api_router(throw_source)
        .fallback_service(serve_assets)
}

// The "fake" configuration will not start the CV pipelines, and will point the
// `analyze-throw` route to simulated throw data.
#[cfg(feature = "fake")]
#[tokio::main]
async fn main() {
    // Build the Axum router.
    let app = create_prod_app(ThrowSource::Simulated);

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
    let app = create_prod_app(ThrowSource::Camera);
    
    // Start the Axum server.
    start_server(app, "0.0.0.0:5001").await;

    // TODO(#7): Implement Clean Shutdown.
}
