use backend_lib::camera::aravis_utils::{PtpConfig, initialize_aravis};
use backend_lib::camera::discovery::get_camera_ids;
use backend_lib::camera::ip_identifier::resolve_iface_to_ip;
use backend_lib::camera::record::run_capture_thread;
use backend_lib::camera::record::writer::{
    Frame, SessionManifest, ensure_dir, write_session_manifest, write_to_disk,
};
use backend_lib::camera::{CameraIngestConfig, CancelableBarrier, RecordWithBothCamerasArgs};
use clap::Parser;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

/// Tool for users to record footage from both cameras using Aravis and
/// store the frames to disk using the command line.
pub fn main() {
    println!("------------------------");
    println!("RECORDING FROM BOTH CAMERAS...");
    println!("------------------------\n");

    let args: RecordWithBothCamerasArgs = RecordWithBothCamerasArgs::parse();
    args.common_args
        .validate()
        .unwrap_or_else(|err| panic!("{err}"));
    let enable_ptp = args.common_args.enable_ptp;
    let max_frames = args.common_args.max_frames;
    let max_duration_s = args.common_args.max_duration_s;
    let throwaway_duration_s = args.common_args.throwaway_duration_s;
    let host_interface_addr = if enable_ptp {
        let ip = resolve_iface_to_ip(args.interface.as_str()).unwrap_or_else(|err| {
            panic!("failed to resolve interface: {:?} (pretty: {})", err, err);
        });
        Some(SocketAddr::new(ip, 0))
    } else {
        None
    };

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
    if camera_ids.len() != 2 {
        eprintln!(
            "The number of cameras found on the network is {}, not 2. Please try again...",
            camera_ids.len(),
        );
        return;
    }

    let left_config = CameraIngestConfig::from_record_both_args(camera_ids[0].clone(), args.clone());
    left_config
        .validate()
        .unwrap_or_else(|err| panic!("{err}"));
    let right_config = CameraIngestConfig::from_record_both_args(camera_ids[1].clone(), args.clone());
    right_config
        .validate()
        .unwrap_or_else(|err| panic!("{err}"));

    write_session_manifest(
        &output_base_dir,
        &SessionManifest {
            left_camera_id: left_config.camera_id.clone(),
            right_camera_id: right_config.camera_id.clone(),
        },
    );

    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_clone1 = Arc::clone(&shutdown);
    let shutdown_clone2 = Arc::clone(&shutdown);

    let ptp_enable_barrier = CancelableBarrier::new(2);
    let ptp_enable_barrier_1 = ptp_enable_barrier.clone();
    let ptp_enable_barrier_2 = ptp_enable_barrier.clone();

    let ptp_configure_barrier = CancelableBarrier::new(2);
    let ptp_configure_barrier_1 = ptp_configure_barrier.clone();
    let ptp_configure_barrier_2 = ptp_configure_barrier.clone();

    let ptp_lock_barrier = CancelableBarrier::new(2);
    let ptp_lock_barrier_1 = ptp_lock_barrier.clone();
    let ptp_lock_barrier_2 = ptp_lock_barrier.clone();

    let configuration_barrier = CancelableBarrier::new(2);
    let configuration_barrier_1 = configuration_barrier.clone();
    let configuration_barrier_2 = configuration_barrier.clone();

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

    let (frame_tx1, frame_rx) = crossbeam::channel::bounded::<Frame>(100);
    let frame_tx2 = frame_tx1.clone();

    let maybe_ptp_config_1 = enable_ptp.then_some(PtpConfig {
        is_slave: false,
        enable_barrier: ptp_enable_barrier_1,
        configure_barrier: ptp_configure_barrier_1,
        lock_barrier: ptp_lock_barrier_1,
    });
    let maybe_ptp_config_2 = enable_ptp.then_some(PtpConfig {
        is_slave: true,
        enable_barrier: ptp_enable_barrier_2,
        configure_barrier: ptp_configure_barrier_2,
        lock_barrier: ptp_lock_barrier_2,
    });

    let record_handle_1 = thread::spawn(move || {
        run_capture_thread(
            Some(output_base_dir),
            &left_config,
            frame_tx1,
            max_frames,
            max_duration_s,
            throwaway_duration_s,
            shutdown_clone1,
            host_interface_addr,
            Some(configuration_barrier_1),
            Some(acquisition_barrier_1),
            maybe_ptp_config_1,
        );
    });

    let record_handle_2 = thread::spawn(move || {
        run_capture_thread(
            Some(output_base_dir_clone),
            &right_config,
            frame_tx2,
            max_frames,
            max_duration_s,
            throwaway_duration_s,
            shutdown_clone2,
            None,
            Some(configuration_barrier_2),
            Some(acquisition_barrier_2),
            maybe_ptp_config_2,
        );
    });

    let writer_handle = thread::spawn(move || {
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

    for handle in [record_handle_1, record_handle_2] {
        handle.join().expect("Error: Recorder thread panicked.");
    }
    writer_handle
        .join()
        .expect("Error: Writer thread panicked.");

    println!("------------------------");
    println!("RECORDING COMPLETE!");
    println!("------------------------\n");
}
