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
use std::time::{Duration, Instant};

// Need both cams to agree on the next timestamp to start recording at
// They're on separate threads and this code architecture doesn't make it easy for them to talk to each other
// So we use math to agree
fn compute_first_frame_ptp_ns(current_ptp_ns: u64) -> u64 {
    let interval_ns: u64 = 10_000_000_000; // 10 seconds in ns
    let margin_ns: u64 = 3_000_000_000; //  3 second  in ns

    let next_boundary = (current_ptp_ns / interval_ns + 1) * interval_ns;

    if next_boundary - current_ptp_ns < margin_ns {
        next_boundary + interval_ns
    } else {
        next_boundary
    }
}

fn unsafe_read_camera_integer(camera: &Camera, node_name: &str) -> i64 {
    unsafe {
        let mut error: *mut glib::ffi::GError = ptr::null_mut();
        let camera_ptr: *mut aravis_sys::ArvCamera = camera.to_glib_none().0;
        let feature_c_str = CString::new(node_name).unwrap();
        let raw_res = arv_camera_get_integer(camera_ptr, feature_c_str.as_ptr(), &mut error);
        if !error.is_null() {
            panic!(
                "Error calling arv_camera_get_integer for node: {}",
                node_name
            );
        }
        raw_res
    }
}

fn read_ptp_time_ns(camera: &Camera) -> u64 {
    camera
        .execute_command("PtpDataSetLatch")
        .expect("Failed to latch PTP dataset.");
    unsafe_read_camera_integer(camera, "PtpDataSetLatchValue") as u64
}

/// Broadcasts a GigEV scheduled action command to all cameras on the network.
/// All values must match what was configured on each camera:
///   device_key, group_key, group_mask, and the scheduled PTP timestamp.
pub fn send_action_command(
    socket: &UdpSocket,
    fire_at_ptp_ns: u64,
    device_key: u32,
    group_key: u32,
    group_mask: u32,
) {
    println!("Sending action command!");
    // GigEV action command packet: 56 bytes total
    // Ref: GigE Vision spec section on Action Commands
    let mut packet = [0u8; 28];

    // GVCP header
    packet[0] = 0x42; // required first byte
    packet[1] = 0b10000001; // flag denotes that action time is being sent and we want an ACK
    packet[2] = 0x01; // command high: ACTION_CMD = 0x0100
    packet[3] = 0x00; // command low
    packet[4] = 0x00; // length high
    packet[5] = 20; // length low (msg beyond header is 20 bytes)
    // request id - can be any nonzero value maybe? Not according to spec but....
    packet[6] = 0x00;
    packet[7] = 0x01;

    // Payload
    // device key
    packet[8] = (device_key >> 24) as u8;
    packet[9] = (device_key >> 16) as u8;
    packet[10] = (device_key >> 8) as u8;
    packet[11] = device_key as u8;

    // group key
    packet[12] = (group_key >> 24) as u8;
    packet[13] = (group_key >> 16) as u8;
    packet[14] = (group_key >> 8) as u8;
    packet[15] = group_key as u8;

    // group mask
    packet[16] = (group_mask >> 24) as u8;
    packet[17] = (group_mask >> 16) as u8;
    packet[18] = (group_mask >> 8) as u8;
    packet[19] = group_mask as u8;

    // scheduled action time in ns (8 bytes, big-endian)
    packet[20] = (fire_at_ptp_ns >> 56) as u8;
    packet[21] = (fire_at_ptp_ns >> 48) as u8;
    packet[22] = (fire_at_ptp_ns >> 40) as u8;
    packet[23] = (fire_at_ptp_ns >> 32) as u8;
    packet[24] = (fire_at_ptp_ns >> 24) as u8;
    packet[25] = (fire_at_ptp_ns >> 16) as u8;
    packet[26] = (fire_at_ptp_ns >> 8) as u8;
    packet[27] = fire_at_ptp_ns as u8;

    // remaining bytes are reserved/zero
    socket
        .send_to(&packet, "255.255.255.255:3956")
        .expect("Failed to send action command.");
}

/// Records stream from a single camera.
pub fn run_capture_thread(
    output_base_dir: PathBuf,
    config: &CameraIngestConfig,
    frame_tx: crossbeam::channel::Sender<Frame>,
    max_frames: Option<usize>,
    max_duration_s: Option<f64>,
    throwaway_duration_s: f64,
    shutdown: Arc<AtomicBool>,
    host_interface_ip: Option<SocketAddr>,
    configuration_barrier: Option<CancelableBarrier>,
    acquisition_barrier: Option<CancelableBarrier>,
    maybe_ptp_config: Option<PtpConfig>,
) {
    println!("Starting capture for camera {}.", config.camera_id);

    // Ensure output directory exists.
    let camera_id = config.camera_id.clone();
    let output_camera_dir = output_base_dir.join(sanitize_path_name(&camera_id));
    ensure_dir(&output_camera_dir);

    // Create Aravis camera, apply configuration, start stream, and queue buffers.
    let camera = match create_camera(&camera_id) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            return;
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

    // For frame metadata.
    let (width, height) = config.resolution.dimensions();

    let first_frame_ptp_ns = if maybe_ptp_config.is_some() {
        let now = read_ptp_time_ns(&camera);
        Some(compute_first_frame_ptp_ns(now))
    } else {
        None
    };
    let frame_interval_ns: Option<u64> = if let Some(ref ptp_config) = maybe_ptp_config
        && !ptp_config.is_slave
    {
        Some((1_000_000_000.0 / config.frame_rate_hz) as u64)
    } else {
        None
    };
    let maybe_socket = if let Some(ref ptp_config) = maybe_ptp_config
        && !ptp_config.is_slave
    {
        let addr = host_interface_ip.expect("Capture thread was configured to be PTP & Acquisition master but was not provided a host SocketAddr.");
        let socket = UdpSocket::bind(addr).expect("Failed to bind UDP socket for action command.");
        socket
            .set_broadcast(true)
            .expect("Failed to enable broadcast on action command socket.");
        Some(socket)
    } else {
        None
    };

    // Start Aravis camera aquisition.
    camera
        .start_acquisition()
        .expect("Failed to start camera acquisition.");
    if maybe_ptp_config.is_some() {
        camera
            .execute_command("TransferStart")
            .expect("Failed to start transfer.");
    }

    if let Some(barrier) = acquisition_barrier {
        if barrier.wait() == BarrierResult::Canceled {
            return;
        }
    }

    if let Some(ref ptp_config) = maybe_ptp_config {
        // Only the Master (PC) schedules actions
        if !ptp_config.is_slave {
            let socket = maybe_socket.expect("Socket missing for PTP master");
            let start_ns = first_frame_ptp_ns.expect("Start time missing");
            let interval_ns = frame_interval_ns.expect("Interval missing");
            let shutdown_heartbeat = shutdown.clone();

            // Sync System Time to PTP Time once
            let p0 = read_ptp_time_ns(&camera);
            let s0 = Instant::now();

            std::thread::spawn(move || {
                let mut frames_pushed = 0u64;
                let lead_time_ns = 2 * interval_ns; // 150ms look-ahead

                loop {
                    if shutdown_heartbeat.load(Ordering::SeqCst) {
                        break;
                    }

                    // Predict current PTP time based on local Instant
                    let elapsed = s0.elapsed().as_nanos() as u64;
                    let estimated_ptp = p0 + elapsed;

                    // Calculate the PTP timestamp for the next frame in the sequence
                    let scheduled_until = start_ns + (frames_pushed * interval_ns);

                    if estimated_ptp > scheduled_until + interval_ns {
                        let frames_behind = (estimated_ptp - scheduled_until) / interval_ns;
                        eprintln!("Action command heartbeat is {frames_behind} frame(s) behind. Local clock may be drifting from PTP time. Skipping {frames_behind} missed frame slots.");
                        // Skip ahead rather than firing a burst of catch-up commands
                        frames_pushed += frames_behind;
                        continue;
                    }

                    // Fill the camera pipeline if we are less than `lead_time_ns` ahead
                    if scheduled_until < (estimated_ptp + lead_time_ns) {
                        send_action_command(&socket, scheduled_until, 1, 1, 1);
                        frames_pushed += 1;
                    } else {
                        // Sleep long enough to avoid pegging the CPU, short enough to keep lead
                        std::thread::sleep(Duration::from_millis(10));
                    }
                }
                println!("Heartbeat thread exiting.");
            });
        }
    }

    // Keep track of start time and the number of saved frames.
    let start_time: Instant = Instant::now();
    let mut frames_saved: usize = 0usize;

    // Used to provide countdowns to the user
    let mut countdown_timer: Instant = Instant::now();
    let expected_mog2_duration_s: f64 = (MOG2_HISTORY_FRAMES as f64) / config.frame_rate_hz;
    let mut mog2_completed_at: Option<Instant> = None;

    // Used to only print timeouts after first buffer arrives.
    let mut first_buffer_arrived = false;

    // Main recording loop.
    loop {
        // Check shutdown flag.
        if shutdown.load(Ordering::SeqCst) {
            println!("Shutting down capture for camera {}.", camera_id);
            break;
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
        // Since the time duration for recording for Mog2 (mog2_duration_s) is an approximation
        // and this would be wrong if frames are dropped, we actually record the instant at
        // which Mog2 frame collection completed, which also accounts for the time throwing
        // away frames.
        if let Some(max_duration_s) = max_duration_s {
            if let Some(mog2_completed_at) = mog2_completed_at {
                if mog2_completed_at.elapsed() >= Duration::from_secs_f64(max_duration_s) {
                    break;
                }
            }
        }

        // If the elapsed time has not passed the throwaway duration, print out to the user
        // every second and skip writing the frame to disk.
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
            // Also be sure to drain the buffer during this time.
            if let Some(buffer) = stream.timeout_pop_buffer(0) {
                stream.push_buffer(buffer);
            }
            continue;
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
                let elapsed_since_start = start_time.elapsed();

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
                        "Frame {} received at {:.2}s for {}.",
                        frames_saved,
                        elapsed_since_start.as_secs_f64(),
                        camera_id,
                    );
                }

                // Take the buffer from the stream and store its information, and then
                // immediately push the buffer back to the stream, so it doesn't
                // starve.
                let data = copy_buffer_bytes(&buffer);
                let system_timestamp_ns = buffer.system_timestamp();
                let buffer_timestamp_ns = buffer.timestamp();
                let frame_id = buffer.frame_id();
                stream.push_buffer(buffer);

                if data.is_empty() {
                    eprintln!("Empty buffer from camera {}.", config.camera_id);
                    continue;
                }

                // Store metadata.
                let metadata = Metadata {
                    camera_id: config.camera_id.clone(),
                    frame_index: frames_saved,
                    width,
                    height,
                    payload_bytes: data.len(),
                    system_timestamp_ns,
                    buffer_timestamp_ns,
                    frame_id,
                };

                // Package bytes and metadata into Frame struct and pass
                // over crossbeam-channel to write thread.
                let frame = Frame {
                    output_camera_dir: output_camera_dir.clone(),
                    frame_index: frames_saved,
                    bytes: data,
                    metadata: metadata,
                };
                frame_tx.send(frame).expect(
                    "Error: Failed to send frame from recording capture thread to write thread.",
                );

                frames_saved += 1;

                if frames_saved == MOG2_HISTORY_FRAMES {
                    mog2_completed_at = Some(Instant::now());
                }
            }
            status => {
                eprintln!(
                    "ERROR: Camera {} returned non-success buffer status: {:?}",
                    config.camera_id, status
                );
            }
        }
    }

    shutdown.store(true, Ordering::SeqCst);

    let _ = camera.stop_acquisition();

    let total_capture_time_s = start_time.elapsed().as_secs_f64() - throwaway_duration_s;
    let frame_rate = frames_saved as f64 / total_capture_time_s;

    println!("\nFinished recording from camera {}.", config.camera_id,);
    println!(
        "Saved {} frame(s) in {:.3} seconds, total frame rate was {:.3} frames per second.",
        frames_saved, total_capture_time_s, frame_rate,
    );
    println!("Wrote files into {}.", output_camera_dir.display());
}
