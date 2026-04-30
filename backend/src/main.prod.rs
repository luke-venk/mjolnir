use axum::Router;
use axum_embed::ServeEmbed;
use backend_lib::circle_infractions_ingest::begin_detecting_circle_infractions;
#[cfg(feature = "real_cameras")]
use backend_lib::aggregator::{AggregationCommand, AggregationCoordinator};
#[cfg(feature = "real_cameras")]
use backend_lib::camera::parse_real_backend_args;
#[cfg(feature = "real_cameras")]
use backend_lib::aggregator::MatchedContourPairAggregator;
#[cfg(feature = "real_cameras")]
use backend_lib::pipeline::{CameraId, Pipeline};
use backend_lib::server::{ThrowSource, create_api_router, start_server};
#[cfg(feature = "real_cameras")]
use backend_lib::pipeline::{Frame, MatchedContourPair};
#[cfg(feature = "real_cameras")]
use backend_lib::aggregator::OptimizeTrajectoryInput;
#[cfg(feature = "real_cameras")]
use clap::Parser;
use rust_embed::Embed;

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
    let args = parse_real_backend_args();

    // Start the 2 computer vision pipelines (one for each camera).
    let rolling_buffer_size: usize = 10;
    let expected_frame_interval_ns = (1_000_000_000.0 / args.frame_rate_hz) as u64;
    let (frame_output_tx, frame_output_rx) =
        crossbeam::channel::bounded::<Frame>(rolling_buffer_size);
    let (matched_pair_tx, matched_pair_rx) =
        crossbeam::channel::bounded::<MatchedContourPair>(rolling_buffer_size);
    let (_aggregation_command_tx, aggregation_command_rx) =
        crossbeam::channel::unbounded::<AggregationCommand>();
    let (optimize_input_tx, _optimize_input_rx) =
        crossbeam::channel::unbounded::<OptimizeTrajectoryInput>();

    let _matched_contour_pair_aggregator = MatchedContourPairAggregator::new(
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
    let app = create_prod_app(ThrowSource::Camera);

    // Start the Axum server.
    start_server(app, "0.0.0.0:5001").await;
}
