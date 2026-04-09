/// Tool for users to record footage from one camera using Aravis and
/// store the frames to disk using the command-line.
use backend_lib::camera::CameraIngestConfig;
use backend_lib::camera::record::cli::RecordFromCamerasArgs;
use backend_lib::camera::record::record_from_one_camera;
use backend_lib::camera::record::writer::{ensure_dir, string_to_pathbuf};
use backend_lib::camera_ingest::{
    ensure_ffmpeg_lossless_hevc_support, record_h265_from_one_camera,
};

use clap::Parser;

pub fn main() {
    println!("------------------------");
    println!("RECORDING FROM ONE CAMERA...");
    println!("------------------------\n");

    // Store command line arguments for recording.
    let args: RecordFromCamerasArgs = RecordFromCamerasArgs::parse();
    args.validate().unwrap_or_else(|err| panic!("{err}"));

    // Create output directory based on command-line argument.
    let output_base_dir = string_to_pathbuf(&args.common_args.output_dir);
    ensure_dir(&output_base_dir);

    let recover_base_dir = args.recover_to_png_dir.as_ref().map(string_to_pathbuf);
    if let Some(recover_base_dir) = recover_base_dir.as_ref() {
        ensure_dir(recover_base_dir);
    }

    // Parse command line arguments into camera ingest config.
    let camera_ingest_config: CameraIngestConfig = CameraIngestConfig::from_record_one_args(args.clone());
    camera_ingest_config
        .validate()
        .unwrap_or_else(|err| panic!("{err}"));

    // Begin recording into one lossless H.265 stream, then optionally recover it to PNGs.
    record_h265_from_one_camera(
        &camera_ingest_config,
        &output_base_dir,
        args.max_frames,
        args.max_duration,
    );

}
