use axum::{Router, http::Method};
use backend_lib::circle_infractions_ingest::begin_detecting_circle_infractions;
#[cfg(feature = "real_cameras")]
use backend_lib::aggregator::{
    AggregationCommand, AggregationCoordinator, MatchedFramePairAggregator, OptimizeTrajectoryInput,
};
#[cfg(feature = "real_cameras")]
use backend_lib::camera::parse_real_backend_args;
#[cfg(feature = "real_cameras")]
use backend_lib::camera_ingest::begin_live_dual_cam_ingest;
#[cfg(feature = "real_cameras")]
use backend_lib::pipeline::{Frame, MatchedFramePair, Pipeline};
use backend_lib::pipeline::CAPACITY_PER_CROSSBEAM_CHANNEL;
use backend_lib::server::{ThrowSource, create_api_router, start_server};
use backend_lib::timing::init_global_time;
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
    init_global_time();
    let app = create_dev_app(ThrowSource::Simulated);
    start_server(app, "0.0.0.0:5001").await;
}

#[cfg(feature = "real_cameras")]
#[tokio::main]
async fn main() {
    init_global_time();
    let args = parse_real_backend_args();

    let expected_frame_interval_ns = (1_000_000_000.0 / 30.0) as u64;
    let (frame_output_tx, frame_output_rx) =
        crossbeam::channel::bounded::<Frame>(CAPACITY_PER_CROSSBEAM_CHANNEL);
    let (matched_pair_tx, matched_pair_rx) =
        crossbeam::channel::bounded::<MatchedFramePair>(CAPACITY_PER_CROSSBEAM_CHANNEL);
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

    if let Some(dir) = args.feed_footage_dir {
        println!(
            "Starting real dev backend in recorded-footage replay mode from {}.",
            dir.display()
        );
        let (left_tx, left_rx) = crossbeam::channel::bounded::<Frame>(CAPACITY_PER_CROSSBEAM_CHANNEL);
        let (right_tx, right_rx) = crossbeam::channel::bounded::<Frame>(CAPACITY_PER_CROSSBEAM_CHANNEL);
        let _replay_handle = std::thread::spawn(move || {
            backend_lib::camera_ingest::replay_recorded_session(dir, left_tx, right_tx);
        });
        let _left_pipeline = Pipeline::from_receiver(
            left_rx,
            CAPACITY_PER_CROSSBEAM_CHANNEL,
            frame_output_tx.clone(),
        );
        let _right_pipeline = Pipeline::from_receiver(
            right_rx,
            CAPACITY_PER_CROSSBEAM_CHANNEL,
            frame_output_tx,
        );
    } else {
        let left_camera_id = args
            .left_camera_id
            .expect("--left-camera-id is required unless --feed-footage-dir is used");
        let right_camera_id = args
            .right_camera_id
            .expect("--right-camera-id is required unless --feed-footage-dir is used");
        let (left_rx, right_rx) = begin_live_dual_cam_ingest(
            left_camera_id,
            right_camera_id,
            args.exposure_time_us,
        );
        let _left_pipeline = Pipeline::from_receiver(
            left_rx,
            CAPACITY_PER_CROSSBEAM_CHANNEL,
            frame_output_tx.clone(),
        );
        let _right_pipeline = Pipeline::from_receiver(
            right_rx,
            CAPACITY_PER_CROSSBEAM_CHANNEL,
            frame_output_tx,
        );
    }

    let app = create_dev_app(ThrowSource::Camera);
    start_server(app, "0.0.0.0:5001").await;
}
