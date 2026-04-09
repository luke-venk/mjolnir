/// Tool for users to record footage from both cameras using Aravis and
/// store the frames to disk using the command-line.
use std::path::PathBuf;
use std::thread;
use clap::Parser;
use backend_lib::camera::CameraIngestConfig;
use backend_lib::camera::aravis_utils::initialize_aravis;
use backend_lib::camera::discovery::get_camera_ids;
use backend_lib::camera::record::cli::RecordWithBothCamerasArgs;
use backend_lib::camera::record::run_capture_thread;
use backend_lib::camera::record::writer::{Frame, ensure_dir, write_to_disk};

pub fn main() {
    println!("------------------------");
    println!("RECORDING FROM BOTH CAMERAS...");
    println!("------------------------\n");

    // Store command line arguments for recording.
    let args: RecordWithBothCamerasArgs = RecordWithBothCamerasArgs::parse();
    args.common_args
        .validate()
        .unwrap_or_else(|err| panic!("{err}"));

    // Create output directory based on command-line argument.
    // TODO: should have output directory include timestamp for beginning
    // of recording, so frames associated with a given throw are stored together.
    let output_base_dir = PathBuf::from(&args.common_args.output_dir);
    let output_base_dir_clone = output_base_dir.clone();
    ensure_dir(&output_base_dir);

    // Stopping condition.
    let max_frames = args.common_args.max_frames;
    let max_duration = args.common_args.max_duration;

    // Find all cameras on the LAN.
    let aravis = initialize_aravis();
    let camera_ids = get_camera_ids(&aravis);

    // Assert that the number of cameras connected is 2.
    if camera_ids.len() != 2 {
        eprintln!(
            "The number of cameras found on the network is {}, not 2. Please try again...",
            camera_ids.len(),
        );
        return;
    }

    // Parse command line arguments into camera ingest configs for each camera.
    let camera_ingest_config1: CameraIngestConfig =
        CameraIngestConfig::from_record_both_args(camera_ids[0].clone(), args.clone());
    camera_ingest_config1
            .validate()
            .unwrap_or_else(|err| panic!("{err}"));
    let camera_ingest_config2: CameraIngestConfig =
        CameraIngestConfig::from_record_both_args(camera_ids[1].clone(), args.clone());
    camera_ingest_config2
            .validate()
            .unwrap_or_else(|err| panic!("{err}"));

    // Create crossbeam channel so capture thread can send frames to
    // write thread. Have a clone of the same sender, as well as the
    // original sender, send to the receiver.
    let (frame_tx1, frame_rx) = crossbeam::channel::bounded::<Frame>(100);
    let frame_tx2 = frame_tx1.clone();

    // Spawn 1st recording thread.
    let record_handle_1 = thread::spawn(move || {
        run_capture_thread(
            output_base_dir,
            &camera_ingest_config1,
            frame_tx1,
            max_frames,
            max_duration,
        );
    });

    // Spawn 2nd recording thread.
    let record_handle_2 = thread::spawn(move || {
        run_capture_thread(
            output_base_dir_clone,
            &camera_ingest_config2,
            frame_tx2,
            max_frames,
            max_duration,
        );
    });

    // Spawn write thread.
    let writer_handle = thread::spawn(move || {
        // Write incoming frames.
        for frame in frame_rx {
            write_to_disk(&frame.output_camera_dir, frame.frame_index, &frame.bytes, &frame.metadata);
        }
    });

    // Prevent main thread from exiting until other threads have exited.
    let record_handles = [record_handle_1, record_handle_2];
    for handle in record_handles {
        handle
            .join()
            .expect("Error: Recorder thread panicked.");
    }

    // Prevent main thread from exiting before writing thread finishes.
    writer_handle.join().expect("Error: Writer thread panicked.");

    // TODO: add feature to just kill both streams cleanly when user wants to quit,
    // that way won't need to guess max duration or max frames.
    println!("------------------------");
    println!("RECORDING COMPLETE!");
    println!("------------------------\n");
}
