use backend_lib::camera::CameraIngestConfig;
use backend_lib::camera::StreamFromCamerasArgs;
use backend_lib::timing::init_global_time;
use clap::Parser;
use eframe::egui;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use stream_lib::capture::run_capture_thread;
use stream_lib::{FrameData, LiveViewApp};

/// Tool for users to stream footage from the cameras using Aravis and
/// tune camera intrinsics quickly, rather than storing raw bytes to
/// disk and converting to PNG afterhand.
///
/// This program requires 2 threads:
///   (1) Main thread: UI rendering
///   (2) Capture thread: Uses Aravis to get frames
/// This is required for one, because capture calls `timeout_pop_buffer`
/// which blocks the capture thread, and two, because macOS specifically
/// requires a main thread, not a background thread, for windowing systems
/// like the egui UI.

fn main() -> eframe::Result<()> {
    init_global_time();
    println!("------------------------");
    println!("LIVE STREAMING FROM CAMERA...");
    println!("------------------------\n");

    // Parse command-line arguments and create camera configuration.
    let args: StreamFromCamerasArgs = StreamFromCamerasArgs::parse();
    let camera_ingest_config: CameraIngestConfig =
        CameraIngestConfig::from_stream_args(args.clone());

    // Create shared camera ingest config object to be shared between the 2 threads.
    let camera_settings = Arc::new(Mutex::new(camera_ingest_config));
    let camera_settings_clone = Arc::clone(&camera_settings);

    // Use crossbeam channel to send the latest frame from the capture thread (sender)
    // to the UI thread (receiver).
    let (frame_tx, frame_rx) = crossbeam::channel::bounded::<FrameData>(100);

    // Shared shutdown flag set by Ctrl+C handler.
    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_clone = Arc::clone(&shutdown);
    ctrlc::set_handler(move || {
        println!("\nShutdown signal received, stopping recording...");
        shutdown_clone.store(true, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl+C handler.");

    // Spawn capture thread.
    thread::spawn(move || {
        run_capture_thread(camera_settings_clone, frame_tx, Arc::clone(&shutdown));
    });

    // Set up eframe.
    let options = eframe::NativeOptions {
        // 983.0 came from 720.0 * 1.365, which is the division of 4096 / 3000 (camera sensor pixels).
        viewport: egui::ViewportBuilder::default()
            .with_title("Mjölnir Live Stream")
            .with_inner_size([983.0, 720.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Mjölnir Live Stream",
        options,
        Box::new(|_cc| Ok(Box::new(LiveViewApp::new(frame_rx, camera_settings)))),
    )
}
