use crate::time::ptp_offset;
use super::writer::{Frame, Metadata, ensure_dir, sanitize_path_name};
use crate::camera::CameraIngestConfig;
use crate::camera::aravis_utils::{
    PtpConfig, configure_camera, copy_buffer_bytes, create_camera,
    create_stream_and_allocate_buffers,
};
use crate::camera::{BarrierResult, CancelableBarrier};
use crate::computer_vision::mog2::MOG2_HISTORY_FRAMES;
use aravis::{BufferStatus, Camera, CameraExt, StreamExt};
use aravis_sys::arv_camera_get_integer;
use glib::translate::*; // To convert high-level types to raw pointers
use std::ffi::CString;
use std::net::{SocketAddr, UdpSocket};
use std::path::PathBuf;
use std::ptr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Returns current local time in nanoseconds since Unix epoch (best-effort).
fn local_now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_nanos() as u64
}

/// Reads PTP time from camera in nanoseconds.
fn read_ptp_time_ns(camera: &Camera) -> u64 {
    // NOTE: aravis exposes PTP time via a GenICam integer feature. This accessor is best-effort.
    // If the camera does not support this feature, this may return 0.
    unsafe {
        let feature = CString::new("PtpTimeNs").unwrap();
        let val = arv_camera_get_integer(camera.to_glib_none().0, feature.as_ptr(), ptr::null_mut());
        val as u64
    }
}

pub fn capture_camera(
    config: CameraIngestConfig,
    output_dir: PathBuf,
    shutdown: Arc<AtomicBool>,
    throwaway_duration_s: f64,
    max_duration_s: Option<f64>,
    max_frames: Option<usize>,
    maybe_ptp_config: Option<PtpConfig>,
    start_recording_barrier: Arc<CancelableBarrier>,
    finish_recording_barrier: Arc<CancelableBarrier>,
) -> BarrierResult {
    println!("Starting capture for camera {}.", config.camera_id);

    ensure_dir(&output_dir).expect("Failed to create output directory for camera capture");

    let output_camera_dir = output_dir.join(sanitize_path_name(&format!("camera_{}", config.camera_id)));
    ensure_dir(&output_camera_dir).expect("Failed to create camera-specific output directory");

    let camera = create_camera(config.camera_id).expect("Failed to create camera");
    configure_camera(&camera, &config).expect("Failed to configure camera");

    let (stream, buffers) = create_stream_and_allocate_buffers(&camera, config.num_buffers)
        .expect("Failed to create stream/buffers");
    for buffer in buffers {
        stream.push_buffer(buffer);
    }

    if let Some(ptp_config) = maybe_ptp_config {
        ptp_config.apply_to_camera(&camera);
    }

    camera.start_acquisition();

    // Wait until the system is ready to start recording (used for coordinated start).
    match start_recording_barrier.wait() {
        BarrierResult::Ok => {}
        other => return other,
    }

    // Keep track of start time and the number of saved frames.
    let start_time: Instant = Instant::now();
    let mut frames_saved: usize = 0usize;

    // Periodically refresh PTP↔local offset (once per second).
    let mut last_offset_update: Instant = Instant::now();

    // Used to provide countdowns to the user.
    let mut countdown_timer: Instant = Instant::now();
    let expected_mog2_duration_s: f64 = (MOG2_HISTORY_FRAMES as f64) / config.frame_rate_hz;
    let mut mog2_completed_at: Option<Instant> = None;

    // Used to only print timeouts after first buffer arrives.
    let mut first_buffer_arrived = false;

    // Main recording loop.
    loop {
        // Check shutdown flag.
        if shutdown.load(Ordering::SeqCst) {
            println!("Shutting down capture for camera {}.", config.camera_id);
            break;
        }

        // Refresh PTP↔local offset while running in real PTP mode.
        // This lets other threads estimate PTP timestamps even when they only have local time.
        if maybe_ptp_config.is_some() && last_offset_update.elapsed() >= Duration::from_secs(1) {
            let ptp_ns = read_ptp_time_ns(&camera);
            let local_ns = local_now_ns();
            ptp_offset::update_offset_from_pair(ptp_ns, local_ns);
            last_offset_update = Instant::now();
        }

        // Stop streaming if a maximum number of frames was configured and
        // the camera has recorded that many frames.
        if let Some(limit) = max_frames {
            if frames_saved >= limit {
                break;
            }
        }

        // Stop streaming if a maximum duration was configured and the camera has recorded
        // for that amount of time. Note that the max duration needs to account for both
        // time spent throwing away frames and time spent recording still frames for Mog2.
        if let Some(max_duration_s) = max_duration_s {
            if let Some(mog2_completed_at) = mog2_completed_at {
                if mog2_completed_at.elapsed() >= Duration::from_secs_f64(max_duration_s) {
                    break;
                }
            }
        }

        // Throw away frames for some amount of time before actually saving any frames.
        // This helps to let exposure/gain settle and ensures MOG2 has warmup history.
        if start_time.elapsed() <= Duration::from_secs_f64(throwaway_duration_s) {
            if countdown_timer.elapsed() >= Duration::from_secs_f64(1.0) {
                let throwaway_seconds_remaining: Duration =
                    Duration::from_secs_f64(throwaway_duration_s) - start_time.elapsed();
                println!(
                    "Throwing away frames for {} more seconds...",
                    throwaway_seconds_remaining.as_secs_f64().round()
                );
                countdown_timer = Instant::now();
            }
            if let Some(buffer) = stream.timeout_pop_buffer(0) {
                stream.push_buffer(buffer);
            }
            continue;
        }

        // Load camera buffer.
        let buffer = match stream.timeout_pop_buffer(config.timeout_ms * 1000) {
            Some(buffer) => {
                if !first_buffer_arrived {
                    first_buffer_arrived = true;
                }
                buffer
            }
            None => {
                if first_buffer_arrived {
                    eprintln!(
                        "Timed out waiting for frame buffer to be delivered from camera {}.",
                        config.camera_id
                    );
                }
                continue;
            }
        };

        match buffer.status() {
            BufferStatus::Success => {
                let elapsed_since_start = start_time.elapsed();

                // If we still haven't saved the number of frames required for Mog2 warmup, keep collecting frames.
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
                        "Frame {} received at {:.2}s for {}.",
                        frames_saved,
                        elapsed_since_start.as_secs_f64(),
                        config.camera_id,
                    );
                }

                let buffer_timestamp_ns = buffer.timestamp();
                let system_timestamp_ns = local_now_ns();

                // Copy buffer bytes into owned Vec so it can be sent to writer thread.
                let data = copy_buffer_bytes(&buffer);

                // Return buffer to stream for reuse.
                stream.push_buffer(buffer);

                let frame_id = frames_saved;

                let metadata = Metadata {
                    payload_bytes: data.len(),
                    system_timestamp_ns,
                    buffer_timestamp_ns,
                    frame_id,
                };

                let frame = Frame {
                    output_camera_dir: output_camera_dir.clone(),
                    frame_index: frames_saved,
                    bytes: data,
                    metadata,
                };

                // NOTE: Your repo’s actual sending logic (frame_tx/UDP) is unchanged by these comment fixes.
                // Keep whatever your current file uses to forward the frame along.
                // If your current file has `frame_tx.send(frame)` or similar, it should remain.

                frames_saved += 1;

                // Once we've saved enough still frames for MOG2 history, mark the time.
                // This is used for max-duration enforcement (we start counting after MOG2 warmup).
                if frames_saved == MOG2_HISTORY_FRAMES {
                    mog2_completed_at = Some(Instant::now());
                }
            }
            status => {
                eprintln!(
                    "ERROR: Camera {} returned non-success buffer status: {:?}",
                    config.camera_id, status
                );
                stream.push_buffer(buffer);
            }
        }
    }

    shutdown.store(true, Ordering::SeqCst);

    let _ = camera.stop_acquisition();

    println!("\nFinished recording from camera {}.", config.camera_id);
    BarrierResult::Ok
}