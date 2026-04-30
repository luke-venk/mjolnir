use axum::{Router, http::Method};
use backend_lib::circle_infractions_ingest::begin_detecting_circle_infractions;
#[cfg(feature = "real_cameras")]
use backend_lib::aggregator::{AggregationCommand, AggregationCoordinator};
#[cfg(feature = "real_cameras")]
use backend_lib::camera::parse_real_backend_args;
#[cfg(feature = "real_cameras")]
use backend_lib::aggregator::MatchedFramePairAggregator;
#[cfg(feature = "real_cameras")]
use backend_lib::pipeline::{CameraId, Pipeline};
use backend_lib::server::{ThrowSource, create_api_router, start_server};
#[cfg(feature = "real_cameras")]
use backend_lib::pipeline::{Frame, MatchedFramePair};
#[cfg(feature = "real_cameras")]
use backend_lib::aggregator::OptimizeTrajectoryInput;
#[cfg(feature = "real_cameras")]
use clap::Parser;
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

    // Start the 2 computer vision pipelines (one for each camera).
    let rolling_buffer_size: usize = 10;
    let expected_frame_interval_ns = (1_000_000_000.0 / args.frame_rate_hz) as u64;
    let (frame_output_tx, frame_output_rx) =
        crossbeam::channel::bounded::<Frame>(rolling_buffer_size);
    let (matched_pair_tx, matched_pair_rx) =
        crossbeam::channel::bounded::<MatchedFramePair>(rolling_buffer_size);
    let (_aggregation_command_tx, aggregation_command_rx) =
        crossbeam::channel::unbounded::<AggregationCommand>();
    let (optimize_input_tx, _optimize_input_rx) =
        crossbeam::channel::unbounded::<OptimizeTrajectoryInput>();

    let _matched_frame_pair_aggregator = MatchedFramePairAggregator::new(
        frame_output_rx,
        matched_pair_tx,
        expected_frame_interval_ns,
    );
    let _aggregation_coordinator = AggregationCoordinator::new(
        matched_pair_rx,
        aggregation_command_rx,
        optimize_input_tx,
        250,
        250,
    );

    let _ = Pipeline::new(
        CameraId::FieldLeft,
        args.left_camera_id,
        rolling_buffer_size,
        frame_output_tx.clone(),
    );
    let _ = Pipeline::new(
        CameraId::FieldRight,
        args.right_camera_id,
        rolling_buffer_size,
        frame_output_tx,
    );

    // Build the Axum router.
    let app = create_dev_app(ThrowSource::Camera);

    // Start the Axum server.
    start_server(app, "0.0.0.0:5001").await;
}
