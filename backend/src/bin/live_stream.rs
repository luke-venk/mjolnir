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
/// 
/// Also note, there are a bunch of different configs now. It could be argued
/// that having both CameraIngestConfig and CameraSettings is redundant, but
/// I think it works for separating what is needed for what (CameraIngestConfig
/// needed for streaming, CameraSettings just needed for user to control 
/// sliders).
use std::sync::{Arc, Mutex};
use std::thread;

use backend_lib::camera::stream::{CameraSettings, FrameData, LiveViewApp};
use backend_lib::camera::stream::capture::run_capture_thread;
use backend_lib::camera::CameraIngestConfig;

use backend_lib::camera::stream::cli::StreamFromCamerasArgs;
use clap::Parser;
use eframe::egui;

fn main() -> eframe::Result<()> {
    println!("------------------------");
    println!("LIVE STREAMING FROM CAMERA...");
    println!("------------------------\n");

    // Parse command-line arguments and create camera configuration.
    let args: StreamFromCamerasArgs = StreamFromCamerasArgs::parse();
    let camera_ingest_config: CameraIngestConfig = CameraIngestConfig::from_stream_args(args.clone());
    let camera_ingest_config_clone: CameraIngestConfig = camera_ingest_config.clone();

    // Create shared latest frame to be shared between the UI thread (this one)
    // and the capture thread.
    let latest_frame: Arc<Mutex<Option<FrameData>>> = Arc::new(Mutex::new(None));
    let latest_frame_clone = Arc::clone(&latest_frame);

    // Create shared camera settings to be shared between the 2 threads.
    let camera_settings = Arc::new(Mutex::new(CameraSettings::new(
        camera_ingest_config.exposure_time_us,
        camera_ingest_config.frame_rate_hz,
    )));
    let camera_settings_clone = camera_settings.clone();

    // Spawn capture thread on new thread.
    thread::spawn(move || {
        run_capture_thread(camera_ingest_config_clone, latest_frame_clone, camera_settings_clone);
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
        Box::new(|_cc| Ok(Box::new(LiveViewApp::new(latest_frame, camera_settings)))),
    )
}
