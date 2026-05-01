use axum::Router;
use axum_embed::ServeEmbed;
#[cfg(feature = "real_cameras")]
use backend_lib::camera::parse_real_backend_args;
use backend_lib::circle_infractions_ingest::begin_detecting_circle_infractions;
#[cfg(feature = "real_cameras")]
use backend_lib::pipeline::start_recorded_footage_pipelines;
use backend_lib::server::{ThrowSource, create_api_router, start_server};
use backend_lib::timing::init_global_time;
use rust_embed::Embed;
#[cfg(feature = "real_cameras")]
use backend_lib::pipeline::{CameraId, Pipeline};
#[cfg(feature = "real_cameras")]
use backend_lib::camera_ingest::begin_live_dual_cam_ingest;
use backend_lib::pipeline::CAPACITY_PER_CROSSBEAM_CHANNEL;

const ARDUINO_BAUD_RATE: u32 = 115200;

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
    create_api_router(throw_source, infractions_rx).fallback_service(serve_assets)
}

// Lacking a "real_cameras" feature flag will not start the CV pipelines, and will point the
// `analyze-throw` route to simulated throw data.
#[cfg(not(feature = "real_cameras"))]
#[tokio::main]
async fn main() {
    init_global_time();
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
    init_global_time();
    let args = parse_real_backend_args();
    if let Some(dir) = args.feed_footage_dir {
        println!(
            "Starting real prod backend in recorded-footage replay mode from {}.",
            dir.display()
        );
        let _ = start_recorded_footage_pipelines(dir, CAPACITY_PER_CROSSBEAM_CHANNEL);
    } else {
        let (left_rx, right_rx) = begin_live_dual_cam_ingest(args.left_camera_id, args.right_camera_id, 10_000.0);
        let _left_pipeline = Pipeline::new(CameraId::FieldLeft, left_rx, CAPACITY_PER_CROSSBEAM_CHANNEL);
        let _right_pipeline = Pipeline::new(CameraId::FieldRight, right_rx, CAPACITY_PER_CROSSBEAM_CHANNEL);
    }

    // Build the Axum router.
    let app = create_prod_app(ThrowSource::Camera);

    // Start the Axum server.
    start_server(app, "0.0.0.0:5001").await;

    // TODO(#7): Implement Clean Shutdown.
}
