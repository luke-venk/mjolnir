use axum::{Router, http::Method};
#[cfg(feature = "real_cameras")]
use backend_lib::camera::parse_real_backend_args;
use backend_lib::circle_infractions_ingest::begin_detecting_circle_infractions;
#[cfg(feature = "real_cameras")]
use backend_lib::pipeline::start_recorded_footage_pipelines;
use backend_lib::server::{ThrowSource, create_api_router, start_server};
use backend_lib::timing::init_global_time;
use std::path::PathBuf;
use tower_http::cors::{Any, CorsLayer};
#[cfg(feature = "real_cameras")]
use backend_lib::pipeline::{CameraId, Pipeline};
#[cfg(feature = "real_cameras")]
use backend_lib::camera_ingest::begin_live_dual_cam_ingest;
use backend_lib::pipeline::CAPACITY_PER_CROSSBEAM_CHANNEL;

const ARDUINO_BAUD_RATE: u32 = 115200;

// In dev mode, the backend can serve the API via command line, and it will
// also serve the Next.js server, so it will need CORS. It will not have any
// embedded assets.
pub fn create_dev_app(throw_source: ThrowSource, frames_dir: Option<PathBuf>) -> Router {
    // Set up CORS layer to allow cross-origin sharing for integration mode.
    // Next.js requests will come from port 3000.
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::HEAD])
        .allow_headers(Any);

    let infractions_rx = begin_detecting_circle_infractions(ARDUINO_BAUD_RATE);

    create_api_router(throw_source, infractions_rx, frames_dir).layer(cors)
}

// Lacking a "real_cameras" feature flag will not start the CV pipelines, and will point the
// `analyze-throw` route to simulated throw data.
#[cfg(not(feature = "real_cameras"))]
#[tokio::main]
async fn main() {
    init_global_time();
    // Build the Axum router.
    let app = create_dev_app(ThrowSource::Simulated, None);

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
    let frames_dir = if let Some(dir) = args.feed_footage_dir {
        println!(
            "Starting real dev backend in recorded-footage replay mode from {}.",
            dir.display()
        );
        let frames_dir = dir.clone();
        let _ = start_recorded_footage_pipelines(dir, CAPACITY_PER_CROSSBEAM_CHANNEL);
        Some(frames_dir)
    } else {
        let (left_rx, right_rx) = begin_live_dual_cam_ingest(args.left_camera_id, args.right_camera_id, 10_000.0);
        let _left_pipeline = Pipeline::new(CameraId::FieldLeft, left_rx, CAPACITY_PER_CROSSBEAM_CHANNEL);
        let _right_pipeline = Pipeline::new(CameraId::FieldRight, right_rx, CAPACITY_PER_CROSSBEAM_CHANNEL);
        None
    };

    // Build the Axum router.
    let app = create_dev_app(ThrowSource::Camera, frames_dir);

    // Start the Axum server.
    start_server(app, "0.0.0.0:5001").await;

    // TODO(#7): Implement Clean Shutdown.
}
