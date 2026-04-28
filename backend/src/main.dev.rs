use axum::{Router, http::Method};
use backend_lib::circle_infractions_ingest::begin_detecting_circle_infractions;
#[cfg(feature = "real_cameras")]
use backend_lib::aggregation_coordinator::{AggregationCommand, AggregationCoordinator};
#[cfg(feature = "real_cameras")]
use backend_lib::camera::CvBackendCameraArgs;
#[cfg(feature = "real_cameras")]
use backend_lib::matched_contour_pair_aggregator::MatchedContourPairAggregator;
#[cfg(feature = "real_cameras")]
use backend_lib::pipeline::{CameraId, Pipeline};
use backend_lib::server::{ThrowSource, create_api_router, start_server};
#[cfg(feature = "real_cameras")]
use backend_lib::schemas::{ContourOutput, MatchedContourPair};
#[cfg(feature = "real_cameras")]
use backend_lib::trajectory_input_collector::OptimizeTrajectoryInput;
#[cfg(feature = "real_cameras")]
use clap::Parser;
use tower_http::cors::{Any, CorsLayer};

const ARDUINO_BAUD_RATE: u32 = 115200;

pub fn create_dev_app(throw_source: ThrowSource) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::HEAD])
        .allow_headers(Any);

    let infractions_rx = begin_detecting_circle_infractions(ARDUINO_BAUD_RATE);

    create_api_router(throw_source, infractions_rx).layer(cors)
}

#[cfg(not(feature = "real_cameras"))]
#[tokio::main]
async fn main() {
    let app = create_dev_app(ThrowSource::Simulated);
    start_server(app, "0.0.0.0:5001").await;
}

#[cfg(feature = "real_cameras")]
#[tokio::main]
async fn main() {
    let args: CvBackendCameraArgs = CvBackendCameraArgs::parse();

    let rolling_buffer_size: usize = 10;
    let (contour_output_tx, contour_output_rx) =
        crossbeam::channel::bounded::<ContourOutput>(rolling_buffer_size);
    let (matched_pair_tx, matched_pair_rx) =
        crossbeam::channel::bounded::<MatchedContourPair>(rolling_buffer_size);
    let (_aggregation_command_tx, aggregation_command_rx) =
        crossbeam::channel::unbounded::<AggregationCommand>();
    let (optimize_input_tx, _optimize_input_rx) =
        crossbeam::channel::unbounded::<OptimizeTrajectoryInput>();

    let _matched_contour_pair_aggregator =
        MatchedContourPairAggregator::new(contour_output_rx, matched_pair_tx, 33_330_000);
    let _aggregation_coordinator = AggregationCoordinator::new(
        matched_pair_rx,
        aggregation_command_rx,
        optimize_input_tx,
        250,
    );

    let _ = Pipeline::new(
        CameraId::FieldLeft,
        args.left_camera_id,
        rolling_buffer_size,
        contour_output_tx.clone(),
    );
    let _ = Pipeline::new(
        CameraId::FieldRight,
        args.right_camera_id,
        rolling_buffer_size,
        contour_output_tx,
    );

    let app = create_dev_app(ThrowSource::Camera);
    start_server(app, "0.0.0.0:5001").await;
}
