use backend_lib::camera::aravis_utils::{PtpConfig, initialize_aravis};
use backend_lib::camera::discovery::get_camera_ids;
use backend_lib::camera::ip_identifier::resolve_iface_to_ip;
use backend_lib::camera::record::run_capture_thread;
use backend_lib::camera::record::writer::{
    Frame, SessionManifest, ensure_dir, write_session_manifest, write_to_disk,
};
use backend_lib::camera::{
    AssignmentInputs, CameraIngestConfig, CancelableBarrier, RecordWithBothCamerasArgs,
    resolve_camera_assignment,
};
use clap::Parser;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

/// Tool for users to record footage from both cameras using Aravis and
/// store the frames to disk using the command-line.

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

    // Resolve which physical camera ID acts as FieldLeft / FieldRight, then
    // build configs and persist the assignment so replay can reproduce it.
    let assignment = resolve_camera_assignment(AssignmentInputs {
        cli_left: args.common_args.left_camera_id.clone(),
        cli_right: args.common_args.right_camera_id.clone(),
        manifest: None,
        available_camera_ids: &camera_ids,
    });
    println!(
        "camera_ingest: recording with left={} right={}",
        assignment.left_camera_id, assignment.right_camera_id
    );

    let camera_ingest_config1: CameraIngestConfig = CameraIngestConfig::from_record_both_args(
        assignment.left_camera_id.clone(),
        args.clone(),
    );
    camera_ingest_config1
        .validate()
        .unwrap_or_else(|err| panic!("{err}"));
    let camera_ingest_config2: CameraIngestConfig = CameraIngestConfig::from_record_both_args(
        assignment.right_camera_id.clone(),
        args.clone(),
    );
    camera_ingest_config2
        .validate()
        .unwrap_or_else(|err| panic!("{err}"));

    write_session_manifest(
        &output_base_dir,
        &SessionManifest {
            left_camera_id: assignment.left_camera_id,
            right_camera_id: assignment.right_camera_id,
        },
    );

    // Create crossbeam channel so capture thread can send frames to
    // write thread. Have a clone of the same sender, as well as the
    // original sender, send to the receiver.
    let (frame_tx1, frame_rx) = crossbeam::channel::bounded::<Frame>(100);
    let frame_tx2 = frame_tx1.clone();

    // Shared shutdown flag set by Ctrl+C handler.
    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_clone1 = Arc::clone(&shutdown);
    let shutdown_clone2 = Arc::clone(&shutdown);

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
        let ptp_config = if args.common_args.enable_ptp {
            Some(ptp_config_1)
        } else {
            None
        };
        run_capture_thread(
            Some(output_base_dir),
            &camera_ingest_config1,
            frame_tx1,
            args.common_args.max_frames,
            args.common_args.max_duration_s,
            args.common_args.throwaway_duration_s,
            Arc::clone(&shutdown_clone1),
            Some(addr),
            Some(configuration_barrier_1),
            Some(acquisition_barrier_1),
            ptp_config,
        );
    });

    // Spawn 2nd recording thread.
    let record_handle_2 = thread::spawn(move || {
        let ptp_config = if args.common_args.enable_ptp {
            Some(ptp_config_2)
        } else {
            None
        };
        run_capture_thread(
            Some(output_base_dir_clone),
            &camera_ingest_config2,
            frame_tx2,
            args.common_args.max_frames,
            args.common_args.max_duration_s,
            args.common_args.throwaway_duration_s,
            Arc::clone(&shutdown_clone2),
            None,
            Some(configuration_barrier_2),
            Some(acquisition_barrier_2),
            ptp_config,
        );
    });

    // Spawn write thread.
    let writer_handle = thread::spawn(move || {
        // Write incoming frames.
        for frame in frame_rx {
            let output_camera_dir = frame
                .output_camera_dir
                .as_ref()
                .expect("recorded two-camera frames should have an output directory");
            write_to_disk(
                output_camera_dir,
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

    // Prevent main thread from exiting before writing thread finishes.
    writer_handle
        .join()
        .expect("Error: Writer thread panicked.");

    println!("------------------------");
    println!("RECORDING COMPLETE!");
    println!("------------------------\n");
}
