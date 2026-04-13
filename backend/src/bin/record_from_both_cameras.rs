/// Tool for users to ingest footage from both cameras into the processing pipeline.
use std::thread;

use aravis::Aravis;
use clap::Parser;

use backend_lib::pipeline::Pipeline;
use backend_lib::schemas::camera_ingest_config::{CameraIngestConfig, Resolution};
use backend_lib::schemas::CameraId;

#[derive(Parser, Debug, Clone)]
#[command(name = "record_from_both_cameras")]
#[command(about = "Feeds both Aravis cameras into camera_ingest and the processing pipeline.")]
pub struct RecordWithBothCamerasArgs {
    #[arg(long = "exposure-us", default_value_t = 10000.0)]
    pub exposure_time_us: f64,

    #[arg(long = "frame-rate-hz", default_value_t = 30.0)]
    pub frame_rate_hz: f64,

    #[arg(long, value_enum, default_value_t = Resolution::UHD4K)]
    pub resolution: Resolution,

    #[arg(long, default_value_t = 16)]
    pub num_buffers: usize,

    #[arg(long, default_value_t = 200)]
    pub timeout_ms: u64,

    #[arg(long)]
    pub output_dir: String,

    #[arg(long)]
    pub max_frames: Option<usize>,

    #[arg(long)]
    pub max_duration: Option<f64>,

    #[arg(long, default_value_t = false)]
    pub enable_ptp: bool,
}

fn get_camera_ids(aravis: &Aravis) -> Vec<String> {
    aravis
        .get_device_list()
        .into_iter()
        .map(|device| device.id.to_string_lossy().into_owned())
        .collect()
}

pub fn main() {
    println!("------------------------");
    println!("RECORDING FROM BOTH CAMERAS...");
    println!("------------------------\n");

    let args: RecordWithBothCamerasArgs = RecordWithBothCamerasArgs::parse();
    let _output_dir = &args.output_dir;

    if args.max_frames.is_none() && args.max_duration.is_none() {
        panic!("You must provide at least one stopping condition: --max-frames or --max-duration");
    }

    let aravis = Aravis::initialize().expect("Failed to initialize Aravis.");
    let camera_ids = get_camera_ids(&aravis);

    if camera_ids.len() != 2 {
        eprintln!(
            "The number of cameras found on the network is {}, not 2. Please try again...",
            camera_ids.len(),
        );
        return;
    }

    let left_config = CameraIngestConfig {
        device_id: camera_ids[0].clone(),
        exposure_time_us: args.exposure_time_us,
        frame_rate_hz: args.frame_rate_hz,
        resolution: args.resolution,
        aperture: None,
        enable_ptp: args.enable_ptp,
        use_fake_interface: false,
        num_buffers: args.num_buffers,
        timeout_ms: args.timeout_ms,
        max_frames: args.max_frames,
        max_duration_s: args.max_duration,
    }
    .validate()
    .unwrap_or_else(|err| panic!("{err}"));

    let right_config = CameraIngestConfig {
        device_id: camera_ids[1].clone(),
        exposure_time_us: args.exposure_time_us,
        frame_rate_hz: args.frame_rate_hz,
        resolution: args.resolution,
        aperture: None,
        enable_ptp: args.enable_ptp,
        use_fake_interface: false,
        num_buffers: args.num_buffers,
        timeout_ms: args.timeout_ms,
        max_frames: args.max_frames,
        max_duration_s: args.max_duration,
    }
    .validate()
    .unwrap_or_else(|err| panic!("{err}"));

    let record_handle_1 = thread::spawn(move || {
        let left_pipeline = Pipeline::from_config(CameraId::FieldLeft, left_config, 100);
        left_pipeline.stop();
    });

    let record_handle_2 = thread::spawn(move || {
        let right_pipeline = Pipeline::from_config(CameraId::FieldRight, right_config, 100);
        right_pipeline.stop();
    });

    for handle in [record_handle_1, record_handle_2] {
        handle.join().expect("Error: Pipeline thread panicked.");
    }

    println!("------------------------");
    println!("RECORDING COMPLETE!");
    println!("------------------------\n");
}
