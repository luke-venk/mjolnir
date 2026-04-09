/// Tool for users to record footage from one camera using Aravis and
/// store the frames to disk using the command-line.
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use clap::Parser;
use backend_lib::camera::CameraIngestConfig;
use backend_lib::camera::record::cli::RecordWithOneCameraArgs;
use backend_lib::camera::record::run_capture_thread;
use backend_lib::camera::record::writer::{Frame, ensure_dir, write_to_disk};

pub fn main() {
    println!("------------------------");
    println!("RECORDING FROM ONE CAMERA...");
    println!("------------------------\n");

    // Store command line arguments for recording.
    let args: RecordWithOneCameraArgs = RecordWithOneCameraArgs::parse();
    args.common_args.validate().unwrap_or_else(|err| panic!("{err}"));

    // Create output directory based on command-line argument.
    let output_base_dir = PathBuf::from(&args.common_args.output_dir);
    ensure_dir(&output_base_dir);

    // Parse command line arguments into camera ingest config.
    let camera_ingest_config: CameraIngestConfig = CameraIngestConfig::from_record_one_args(args.clone());
    camera_ingest_config
        .validate()
        .unwrap_or_else(|err| panic!("{err}"));
    
    // Create crossbeam channel so capture thread can send frames to
    // write thread.
    let (frame_tx, frame_rx) = crossbeam::channel::bounded::<Frame>(100);

    // Shared shutdown flag set by Ctrl+C handler.
    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_clone = Arc::clone(&shutdown);
    ctrlc::set_handler(move || {
        println!("\nShutdown signal received, stopping recording...");
        shutdown_clone.store(true, Ordering::SeqCst);
    }).expect("Error setting Ctrl+C handler.");

    // Spawn capture thread.
    let record_handle = thread::spawn(move || {
        run_capture_thread(
            output_base_dir,
            &camera_ingest_config,
            frame_tx,
            args.common_args.max_frames,
            args.common_args.max_duration,
            Arc::clone(&shutdown),
        );
    });

    // Spawn write thread.
    let writer_handle = thread::spawn(move || {
        // Write incoming frames.
        for frame in frame_rx {
            write_to_disk(&frame.output_camera_dir, frame.frame_index, &frame.bytes, &frame.metadata);
        }
    });

    // Prevent main thread from exiting before recording thread finishes.
    record_handle.join().expect("Error: Recorder thread panicked.");

    // Prevent main thread from exiting before writing thread finishes.
    writer_handle.join().expect("Error: Writer thread panicked.");

}
