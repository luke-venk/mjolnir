use crate::camera::aravis_utils::{
    configure_camera, copy_buffer_bytes, create_camera, create_stream_and_allocate_buffers,
    initialize_aravis, PtpConfig,
};
use crate::camera::{AtlasATP124SResolution, BarrierResult, CameraIngestConfig, CancelableBarrier};
use crate::computer_vision::mog2::MOG2_HISTORY_FRAMES;
use crate::pipeline::{CameraId, Context, Frame, CAPACITY_PER_CROSSBEAM_CHANNEL};
use aravis::{BufferStatus, CameraExt, StreamExt};
use crossbeam::channel::{bounded, Receiver};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

pub fn begin_live_dual_cam_ingest(
    left_cam_id: String,
    right_cam_id: String,
    exposure_time_us: f64,
    shutdown_rx: Receiver<()>,
) -> (Receiver<Frame>, Receiver<Frame>) {
    let _ = initialize_aravis();

    let left_cam_ingest_config = CameraIngestConfig {
        camera_id: left_cam_id,
        exposure_time_us,
        frame_rate_hz: 42.5,
        resolution: AtlasATP124SResolution::Full,
        num_buffers: 16,
        timeout_ms: 5000,
        restart_requested: false,
    };

    let right_cam_ingest_config = CameraIngestConfig {
        camera_id: right_cam_id,
        exposure_time_us,
        frame_rate_hz: 42.5,
        resolution: AtlasATP124SResolution::Full,
        num_buffers: 16,
        timeout_ms: 5000,
        restart_requested: false,
    };

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

    std::thread::spawn(move || {
        let _ = shutdown_rx.recv();
        println!("\nShutdown signal received, stopping recording...");
        shutdown.store(true, Ordering::SeqCst);
        ptp_enable_barrier.cancel();
        ptp_configure_barrier.cancel();
        ptp_lock_barrier.cancel();
        configuration_barrier.cancel();
        acquisition_barrier.cancel();
    });

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

    let (tx_left, rx_left) = bounded::<Frame>(CAPACITY_PER_CROSSBEAM_CHANNEL);
    let (tx_right, rx_right) = bounded::<Frame>(CAPACITY_PER_CROSSBEAM_CHANNEL);

    // Spawn 1st streaming thread.
    let _ = thread::spawn(move || {
        run_capture_thread(
            &left_cam_ingest_config,
            CameraId::FieldLeft,
            tx_left,
            1.0,
            Arc::clone(&shutdown_clone1),
            Some(configuration_barrier_1),
            Some(acquisition_barrier_1),
            Some(ptp_config_1),
        );
    });

    // Spawn 2nd streaming thread.
    let _ = thread::spawn(move || {
        run_capture_thread(
            &right_cam_ingest_config,
            CameraId::FieldRight,
            tx_right,
            1.0,
            Arc::clone(&shutdown_clone2),
            Some(configuration_barrier_2),
            Some(acquisition_barrier_2),
            Some(ptp_config_2),
        );
    });

    (rx_left, rx_right)
}

/// Records stream from a single camera.
pub fn run_capture_thread(
    config: &CameraIngestConfig,
    camera_left_or_right: CameraId,
    frame_tx: crossbeam::channel::Sender<Frame>,
    throwaway_duration_s: f64,
    shutdown: Arc<AtomicBool>,
    configuration_barrier: Option<CancelableBarrier>,
    acquisition_barrier: Option<CancelableBarrier>,
    maybe_ptp_config: Option<PtpConfig>,
) {
    println!("Starting capture for camera {}.", config.camera_id);

    let camera_id = config.camera_id.clone();

    // Create Aravis camera, apply configuration, start stream, and queue buffers.
    let camera = match create_camera(&camera_id) {
        Ok(c) => c,
        Err(e) => {
            panic!("{e}");
        }
    };
    configure_camera(
        &camera,
        &config,
        Some(shutdown.clone()),
        maybe_ptp_config.as_ref(),
    );

    if let Some(barrier) = configuration_barrier {
        if barrier.wait() == BarrierResult::Canceled {
            return;
        }
    }

    let stream = create_stream_and_allocate_buffers(&camera, config.num_buffers);

    // Start Aravis camera aquisition.
    camera
        .start_acquisition()
        .expect("Failed to start camera acquisition.");

    if let Some(barrier) = acquisition_barrier {
        if barrier.wait() == BarrierResult::Canceled {
            return;
        }
    }

    // Keep track of start time and the number of frames saved/dropped.
    let start_time: Instant = Instant::now();
    let mut frames_saved: usize = 0usize;
    let mut frames_dropped: usize = 0usize;

    // Define recording start time separately so can properly compute frame rate.
    let mut recording_start_time: Option<Instant> = None;

    // Used to provide countdowns to the user
    let mut countdown_timer: Instant = Instant::now();
    let expected_mog2_duration_s: f64 = (MOG2_HISTORY_FRAMES as f64) / config.frame_rate_hz;

    // Used to only print timeouts after first buffer arrives.
    let mut first_buffer_arrived = false;

    // Main recording loop.
    loop {
        // Check shutdown flag.
        if shutdown.load(Ordering::SeqCst) {
            println!("Shutting down capture for camera {}.", camera_id);
            break;
        }

        // If the elapsed time has not passed the throwaway duration, print out to the user
        // every second and skip writing the frame to disk.
        if start_time.elapsed() <= Duration::from_secs_f64(throwaway_duration_s) {
            if countdown_timer.elapsed() >= Duration::from_secs_f64(1.0) {
                let throwaway_seconds_remaining: Duration =
                    Duration::from_secs_f64(throwaway_duration_s)
                        .saturating_sub(start_time.elapsed());
                println!(
                    "Throwing away frames for {} more seconds...",
                    throwaway_seconds_remaining.as_secs_f64().round()
                );
                countdown_timer = Instant::now();
            }
            // Also be sure to drain the buffer during this time.
            if let Some(buffer) = stream.timeout_pop_buffer(0) {
                stream.push_buffer(buffer);
            }
            continue;
        } else if recording_start_time.is_none() {
            // If elapsed time has passed the throwaway duration and the recording start
            // time hasn't been set, set it.
            recording_start_time = Some(Instant::now());
        }

        // Load camera buffer.
        // Block current thread until frame buffer delivered or the timeout elapses.
        let buffer = match stream.timeout_pop_buffer(config.timeout_ms * 1000) {
            Some(buffer) => {
                if !first_buffer_arrived {
                    first_buffer_arrived = true;
                }
                buffer
            }
            None => {
                // If the buffer hasn't yet loaded, skip.
                // At startup, buffers won't be loaded because between `start_acquisition()`
                // and the first real frame arriving, camera has to arm sensors, establish
                // network path, complete exposure time, etc. So to prevent messy printing,
                // only print the below if a timeout occurs after the first buffer arrives.
                if first_buffer_arrived {
                    frames_dropped += 1;
                    eprintln!(
                        "Timed out waiting for frame buffer to be delivered from camera {}.",
                        config.camera_id
                    );
                }
                continue;
            }
        };

        // If loading the buffer worked, copy the frame buffer into raw bytes.
        match buffer.status() {
            BufferStatus::Success => {
                let buffer_timestamp_ns = buffer.timestamp();

                // If we still haven't saved the number of frames required for Mog2 to
                // build the background model, continue writing to disk and inform the
                // user every second of this, so they can remain still.
                // We just check number of frames to determine if we've hit our frame
                // limit or not, but use a calculated duration to inform the user of how
                // much longer they shoudl wait.
                if frames_saved < MOG2_HISTORY_FRAMES {
                    if countdown_timer.elapsed() >= Duration::from_secs_f64(1.0) {
                        let mog2_seconds_remaining: Duration = Duration::from_secs_f64(
                            expected_mog2_duration_s + throwaway_duration_s,
                        )
                        .saturating_sub(start_time.elapsed());
                        println!(
                            "Recording background frames for Mog2. Remain still for *approximately* {} more seconds...",
                            mog2_seconds_remaining.as_secs_f64().round()
                        );
                        countdown_timer = Instant::now();
                    }
                } else {
                    println!(
                        "Frame {} captured at {}ms for {}.",
                        frames_saved,
                        buffer_timestamp_ns / 1_000_000,
                        camera_id,
                    );
                }

                // Take the buffer from the stream and store its information, and then
                // immediately push the buffer back to the stream, so it doesn't
                // starve.
                let data = copy_buffer_bytes(&buffer);
                stream.push_buffer(buffer);

                if data.is_empty() {
                    eprintln!("Empty buffer from camera {}.", config.camera_id);
                    continue;
                }

                // Package bytes and metadata into Frame struct and pass
                // over crossbeam-channel to write thread.
                let frame = Frame::new(
                    data.into_boxed_slice(),
                    AtlasATP124SResolution::Full.dimensions(),
                    Context::new(camera_left_or_right, buffer_timestamp_ns),
                );
                frame_tx.send(frame).expect(
                    "Error: Failed to send frame from recording capture thread to write thread.",
                );

                frames_saved += 1;
            }
            status => {
                frames_dropped += 1;
                eprintln!(
                    "ERROR: Camera {} returned non-success buffer status: {:?}",
                    config.camera_id, status
                );
            }
        }
    }

    // Stop acquisition.
    shutdown.store(true, Ordering::SeqCst);
    let _ = camera.stop_acquisition();

    // Compute how much time has passed since recording has started.
    // Then, report metrics.
    if let Some(record_start) = recording_start_time {
        let total_capture_time_s: f64 = record_start.elapsed().as_secs_f64();
        let effective_frame_rate: f64 = frames_saved as f64 / total_capture_time_s;
        let delivery_rate: f64 = frames_saved as f64 / (frames_saved + frames_dropped) as f64;

        println!("Finished livestreaming from camera {}.", config.camera_id,);
        println!();
        println!(
            "Saved {} frame(s) and dropped {} frames(s) in {:.3} seconds.",
            frames_saved, frames_dropped, total_capture_time_s,
        );
        println!(
            "The effective frame rate was {:.3} FPS (requested {:.3} FPS). Delivery rate was {:.1}%.",
            effective_frame_rate, config.frame_rate_hz, delivery_rate * 100.0,
        );
    } else {
        println!("Recording was cancelled before any frames were written.");
    }
}
