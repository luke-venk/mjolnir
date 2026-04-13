// Tool for users to record footage from both cameras using Aravis and
// feed the captured frames through camera ingest into the pipeline.
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use std::thread;
use clap::Parser;
use backend_lib::camera::CameraIngestConfig;
use backend_lib::camera::aravis_utils::initialize_aravis;
use backend_lib::camera::discovery::get_camera_ids;
use backend_lib::camera_ingest::run_recording_ingest;
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

    // Create output directory based on command-line argument, with timestamp
    // so each recording session is stored in its own directory.
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Error: Failed to get system time.")
        .as_secs();
    let output_base_dir = PathBuf::from(&args.common_args.output_dir).join(timestamp.to_string());
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
    let camera_id1 = camera_ingest_config1.camera_id.clone();
    let camera_id2 = camera_ingest_config2.camera_id.clone();

    // Create crossbeam channel so capture thread can send frames to
    // fan-out thread. Have a clone of the same sender, as well as the
    // original sender, send to the receiver.
    let (frame_tx1, frame_rx) = crossbeam::channel::bounded::<Frame>(100);
    let frame_tx2 = frame_tx1.clone();
    
    // The fan-out thread duplicates each captured frame so one copy can
    // go to camera ingest and another copy can still be written to disk.
    let (ingest_tx, ingest_rx) = crossbeam::channel::bounded::<Frame>(100);
    let (writer_tx, writer_rx) = crossbeam::channel::bounded::<Frame>(100);

    // Shared shutdown flag set by Ctrl+C handler.
    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_clone1 = Arc::clone(&shutdown);
    let shutdown_clone2 = Arc::clone(&shutdown);
    
    ctrlc::set_handler(move || {
        println!("\nShutdown signal received, stopping recording...");
        shutdown.store(true, Ordering::SeqCst);
    }).expect("Error setting Ctrl+C handler.");

    // Spawn 1st recording thread.
    let record_handle_1 = thread::spawn(move || {
        run_capture_thread(
            output_base_dir,
            &camera_ingest_config1,
            frame_tx1,
            max_frames,
            max_duration,
            Arc::clone(&shutdown_clone1),
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
            Arc::clone(&shutdown_clone2),
        );
    });

    // Spawn fan-out thread.
    let dispatch_handle = thread::spawn(move || {
        for frame in frame_rx {
            if writer_tx.send(frame.clone()).is_err() {
                break;
            }

            if ingest_tx.send(frame).is_err() {
                break;
            }
        }
    });

    // Spawn ingest thread.
    let ingest_handle = thread::spawn(move || {
        run_recording_ingest(ingest_rx, camera_id1, camera_id2, 100);
    });

    // Spawn write thread.
    let writer_handle = thread::spawn(move || {
        for frame in writer_rx {
            write_to_disk(&frame.output_camera_dir,frame.frame_index,&frame.bytes,&frame.metadata,
            );
        }
    });

    // Prevent main thread from exiting until other threads have exited.
    let record_handles = [record_handle_1, record_handle_2];
    for handle in record_handles {
        handle
            .join()
            .expect("Error: Recorder thread panicked.");
    }
    dispatch_handle
        .join()
        .expect("Error: Dispatch thread panicked.");
    ingest_handle
        .join()
        .expect("Error: Ingest dispatch thread panicked.");
    writer_handle
        .join()
        .expect("Error: Writer thread panicked.");

    println!("------------------------");
    println!("RECORDING COMPLETE!");
    println!("------------------------\n");
}
