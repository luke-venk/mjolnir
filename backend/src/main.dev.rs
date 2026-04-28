use axum::{http::Method, Router};
use backend_lib::circle_infractions_ingest::begin_detecting_circle_infractions;
#[cfg(feature = "real_cameras")]
use backend_lib::camera::aravis_utils::initialize_aravis;
#[cfg(feature = "real_cameras")]
use backend_lib::camera::discovery::get_camera_ids;
#[cfg(feature = "real_cameras")]
use backend_lib::camera::{CameraIngestConfig, parse_real_backend_args};
#[cfg(feature = "real_cameras")]
use backend_lib::pipeline::{start_recorded_footage_pipelines, start_recording_camera_pipelines};
use backend_lib::server::{create_api_router, start_server, ThrowSource};
use tower_http::cors::{Any, CorsLayer};

const ARDUINO_BAUD_RATE: u32 = 115200;

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

    let infractions_rx = begin_detecting_circle_infractions(ARDUINO_BAUD_RATE);

    create_api_router(throw_source, infractions_rx).layer(cors)
}

// Lacking a "real_cameras" feature flag will not start the CV pipelines, and will point the
// `analyze-throw` route to simulated throw data.
#[cfg(not(feature = "real_cameras"))]
#[tokio::main]
async fn main() {
    // Build the Axum router.
    let app = create_dev_app(ThrowSource::Simulated);

    // Start the Axum server.
    start_server(app, "0.0.0.0:5001").await;
}

// The "real_cameras" configuration will start the CV pipelines, and will point the
// `analyze-throw` route to the processed throw data from the pipelines.
#[cfg(feature = "real_cameras")]
#[tokio::main]
async fn main() {
    let args = parse_real_backend_args();
    args.validate().unwrap_or_else(|err| panic!("{err}"));
    let rolling_buffer_size: usize = 10;

    if let Some(footage_dir) = args.feed_footage_dir {
        println!(
            "Starting real dev backend in recorded-footage replay mode from {}.",
            footage_dir.display()
        );
        let _ = start_recorded_footage_pipelines(footage_dir, rolling_buffer_size);
    } else {
        // Start the 2 camera-ingest + pipeline flows (one for each camera).
        let aravis = initialize_aravis();
        let camera_ids = get_camera_ids(&aravis);
        assert_eq!(
            camera_ids.len(),
            2,
            "expected exactly 2 cameras for real dev mode, found {}",
            camera_ids.len()
        );

        let left_config = CameraIngestConfig::from_real_args(camera_ids[0].clone(), &args);
        let right_config = CameraIngestConfig::from_real_args(camera_ids[1].clone(), &args);
        let _ = start_recording_camera_pipelines(
            args.interface.as_deref(),
            left_config,
            right_config,
            rolling_buffer_size,
        );
    }

    // Build the Axum router.
    let app = create_dev_app(ThrowSource::Camera);

    // Start the Axum server.
    start_server(app, "0.0.0.0:5001").await;

    // TODO(#7): Implement Clean Shutdown.
}
