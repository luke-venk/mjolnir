/// Tool for users to record footage from both cameras using Aravis and
/// store the frames to disk using the command-line.
use std::thread;

use backend_lib::camera::CameraIngestConfig;
use backend_lib::camera::aravis_utils::initialize_aravis;
use backend_lib::camera::discovery::get_camera_ids;
use backend_lib::camera::record::cli::RecordWithBothCamerasArgs;
use backend_lib::camera::record::compression::ensure_ffmpeg_lossless_hevc_support;
use backend_lib::camera::record::record_from_one_camera;
use backend_lib::camera::record::writer::{ensure_dir, string_to_pathbuf};

use clap::Parser;

pub fn main() {
    println!("------------------------");
    println!("RECORDING FROM BOTH CAMERAS...");
    println!("------------------------\n");

    // Store command line arguments for recording.
    let args: RecordWithBothCamerasArgs = RecordWithBothCamerasArgs::parse();
    args.common_args
        .validate()
        .unwrap_or_else(|err| panic!("{err}"));

    if args.common_args.compress {
        ensure_ffmpeg_lossless_hevc_support().unwrap_or_else(|err| panic!("{err:#}"));
    }

    // Create output directory based on command-line argument.
    let output_base_dir = string_to_pathbuf(&args.common_args.output_dir);
    ensure_dir(&output_base_dir);

    let recover_base_dir = args
        .common_args
        .recover_to_png_dir
        .as_ref()
        .map(string_to_pathbuf);
    if let Some(recover_base_dir) = recover_base_dir.as_ref() {
        ensure_dir(recover_base_dir);
    }

    // Find all cameras on the LAN.
    let aravis = initialize_aravis();
    let camera_ids = get_camera_ids(&aravis);

    if camera_ids.is_empty() {
        println!(
            "No cameras were found on the network to record with. Please try recording/streaming again..."
        );
        return;
    }

    // Store thread handles for each camera recording so main thread doesn't exit.
    let mut handles = vec![];

    // Create camera configs for each camera based on CLI args and
    // spawn a recording thread for each.
    for camera_id in camera_ids {
        let camera_ingest_config: CameraIngestConfig =
            CameraIngestConfig::from_record_both_args(camera_id, args.clone());
        camera_ingest_config
            .validate()
            .unwrap_or_else(|err| panic!("{err}"));

        let output_path = output_base_dir.clone();
        let recover_path = recover_base_dir.clone();
        let max_frames = args.common_args.max_frames;
        let max_duration = args.common_args.max_duration;
        let join_handle = thread::spawn(move || {
            record_from_one_camera(
                &camera_ingest_config,
                &output_path,
                recover_path.as_ref(),
                max_frames,
                max_duration,
            );
        });
        handles.push(join_handle);
    }

    // Prevent main thread from exiting until other threads have exited.
    for handle in handles {
        handle
            .join()
            .expect("Error: Failed to join recording thread.");
    }

    println!("------------------------");
    println!("RECORDING COMPLETE!");
    println!("------------------------\n");
}
