/// Tool for users to record footage from one camera using Aravis and
/// store the frames to disk using the command-line.
use backend_lib::camera::CameraIngestConfig;
use backend_lib::camera::record::cli::RecordWithOneCameraArgs;
use backend_lib::camera::record::record_from_one_camera;
use backend_lib::camera::record::writer::{ensure_dir, string_to_pathbuf};

use clap::Parser;

pub fn main() {
    println!("------------------------");
    println!("RECORDING FROM ONE CAMERA...");
    println!("------------------------\n");

    // Store command line arguments for recording.
    let args: RecordWithOneCameraArgs = RecordWithOneCameraArgs::parse();
    args.common_args.validate().unwrap_or_else(|err| panic!("{err}"));

    // Create output directory based on command-line argument.
    let output_base_dir = string_to_pathbuf(&args.common_args.output_dir);
    ensure_dir(&output_base_dir);

    // Parse command line arguments into camera ingest config.
    let camera_ingest_config: CameraIngestConfig = CameraIngestConfig::from_record_one_args(args.clone());
    camera_ingest_config
        .validate()
        .unwrap_or_else(|err| panic!("{err}"));

    // Begin recording.
    record_from_one_camera(
        &camera_ingest_config,
        &output_base_dir,
        args.common_args.max_frames,
        args.common_args.max_duration,
    );
}
