use backend_lib::camera::aravis_utils::{PtpConfig, initialize_aravis};
use backend_lib::camera::discovery::get_camera_ids;
use backend_lib::camera::ip_identifier::resolve_iface_to_ip;
use backend_lib::camera::record::run_capture_thread;
use backend_lib::camera::record::writer::{Frame, ensure_dir, write_to_disk};
use backend_lib::camera::{CameraIngestConfig, CancelableBarrier, RecordWithBothCamerasArgs};
use backend_lib::camera_ingest::run_recording_ingest;
use backend_lib::pipeline::Pipeline;
use backend_lib::schemas::{CameraId, Frame as PipelineFrame};
use clap::Parser;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

/// Tool for users to record footage from both cameras using Aravis,
/// store the frames to disk, and feed the recorded stream into camera ingest.
pub fn main() {
    println!("------------------------");
    println!("RECORDING FROM BOTH CAMERAS...");
    println!("------------------------\n");

    // Store command line arguments for recording.
    let args: RecordWithBothCamerasArgs = RecordWithBothCamerasArgs::parse();
    args.common_args
        .validate()
        .unwrap_or_else(|err| panic!("{err}"));
    let ip = resolve_iface_to_ip(args.interface.as_str()).unwrap_or_else(|e| {
        panic!("failed to resolve interface: {:?} (pretty: {})", e, e);
    });
    let addr = SocketAddr::new(ip, 0);

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

    // Create crossbeam channel so capture threads can send frames to
    // the fan-out thread.
    let (frame_tx1, frame_rx) = crossbeam::channel::bounded::<Frame>(100);
    let frame_tx2 = frame_tx1.clone();

    // The fan-out thread duplicates each captured frame so one copy can
    // go to camera ingest and another copy can still be written to disk.
    let (ingest_tx, ingest_rx) = crossbeam::channel::bounded::<Frame>(100);
    let (writer_tx, writer_rx) = crossbeam::channel::bounded::<Frame>(100);
    let (left_pipeline_tx, left_pipeline_rx) = crossbeam::channel::bounded::<PipelineFrame>(100);
    let (right_pipeline_tx, right_pipeline_rx) =
        crossbeam::channel::bounded::<PipelineFrame>(100);
    let left_pipeline = Pipeline::new(CameraId::FieldLeft, left_pipeline_rx, 100);
    let right_pipeline = Pipeline::new(CameraId::FieldRight, right_pipeline_rx, 100);

    // Shared shutdown flag set by Ctrl+C handler.
    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_clone1 = Arc::clone(&shutdown);
    let shutdown_clone2 = Arc::clone(&shutdown);
    let shutdown_clone3 = Arc::clone(&shutdown);

    // So we can make capture threads wait until all have PTP enabled, and cancel a wait in the event of SIGINT
    let ptp_enable_barrier = CancelableBarrier::new(2);
    let ptp_enable_barrier_1 = ptp_enable_barrier.clone();
    let ptp_enable_barrier_2 = ptp_enable_barrier.clone();
    // So we can make capture threads wait until all have applied their PTP configuration settings, and cancel a wait in the event of SIGINT
    let ptp_configure_barrier = CancelableBarrier::new(2);
    let ptp_configure_barrier_1 = ptp_configure_barrier.clone();
    let ptp_configure_barrier_2 = ptp_configure_barrier.clone();
    // So we can make capture threads wait until all have achieved PTP lock, and cancel a wait in the event of SIGINT
    let ptp_lock_barrier = CancelableBarrier::new(2);
    let ptp_lock_barrier_1 = ptp_lock_barrier.clone();
    let ptp_lock_barrier_2 = ptp_lock_barrier.clone();
    // So we can make capture threads wait until all have configuration beyond PTP (like exposure, resolution, etc), and cancel a wait in the event of SIGINT
    let configuration_barrier = CancelableBarrier::new(2);
    let configuration_barrier_1 = configuration_barrier.clone();
    let configuration_barrier_2 = configuration_barrier.clone();
    // So we can make capture threads wait until all have started acquisition, and cancel a wait in the event of SIGINT
    let acquisition_barrier = CancelableBarrier::new(2);
    let acquisition_barrier_1 = acquisition_barrier.clone();
    let acquisition_barrier_2 = acquisition_barrier.clone();

    ctrlc::set_handler(move || {
        println!("\nShutdown signal received, stopping recording...");
        shutdown.store(true, Ordering::SeqCst);
        ptp_enable_barrier.cancel();
        ptp_configure_barrier.cancel();
        ptp_lock_barrier.cancel();
        configuration_barrier.cancel();
        acquisition_barrier.cancel();
    })
    .expect("Error setting Ctrl+C handler.");

    let ptp_config_1 = PtpConfig {
        is_slave: false,
        enable_barrier: ptp_enable_barrier_1,
        configure_barrier: ptp_configure_barrier_1,
        lock_barrier: ptp_lock_barrier_1,
    };
    let ptp_config_2 = PtpConfig {
        is_slave: true,
        enable_barrier: ptp_enable_barrier_2,
        configure_barrier: ptp_configure_barrier_2,
        lock_barrier: ptp_lock_barrier_2,
    };

    // Spawn 1st recording thread.
    let record_handle_1 = thread::spawn(move || {
        run_capture_thread(
            output_base_dir,
            &camera_ingest_config1,
            frame_tx1,
            max_frames,
            max_duration,
            Arc::clone(&shutdown_clone1),
            Some(addr),
            Some(configuration_barrier_1),
            Some(acquisition_barrier_1),
            Some(ptp_config_1),
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
            None,
            Some(configuration_barrier_2),
            Some(acquisition_barrier_2),
            Some(ptp_config_2),
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
        run_recording_ingest(
            ingest_rx,
            camera_id1,
            camera_id2,
            left_pipeline_tx,
            right_pipeline_tx,
            shutdown_clone3,
        );
    });

    // Spawn write thread.
    let writer_handle = thread::spawn(move || {
        for frame in writer_rx {
            write_to_disk(
                &frame.output_camera_dir,
                frame.frame_index,
                &frame.bytes,
                &frame.metadata,
            );
        }
    });

    // Prevent main thread from exiting until other threads have exited.
    let record_handles = [record_handle_1, record_handle_2];
    for handle in record_handles {
        handle.join().expect("Error: Recorder thread panicked.");
    }

    dispatch_handle
        .join()
        .expect("Error: Dispatch thread panicked.");
    ingest_handle
        .join()
        .expect("Error: Ingest dispatch thread panicked.");
    left_pipeline.stop();
    right_pipeline.stop();
    writer_handle
        .join()
        .expect("Error: Writer thread panicked.");

    println!("------------------------");
    println!("RECORDING COMPLETE!");
    println!("------------------------\n");
}
