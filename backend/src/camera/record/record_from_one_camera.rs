use std::path::PathBuf;
use std::time::{Duration, Instant};

use super::CameraIngestConfig;
use super::writer::{FrameMetadata, ensure_dir, sanitize_path_name, write_frame_files};
use crate::camera::aravis_utils::{
    configure_camera, copy_buffer_bytes, create_camera, create_stream_and_allocate_buffers,
};

use aravis::{BufferStatus, CameraExt, StreamExt};

/// Records stream from a single camera.
pub fn record_from_one_camera(
    config: &CameraIngestConfig,
    output_base_dir: &PathBuf,
    max_frames: Option<usize>,
    max_duration: Option<f64>,
) {
    // Ensures that output directory for this specific camera exists.
    let output_camera_dir = output_base_dir.join(sanitize_path_name(&config.camera_id));
    ensure_dir(&output_camera_dir);

    // Create Aravis camera, apply configuration, start stream, and queue buffers.
    let camera = create_camera(&config.camera_id);
    configure_camera(&camera, &config);
    let stream = create_stream_and_allocate_buffers(&camera, config.num_buffers);

    // Print to user that camera has been configured.
    let (_, _, width, height) = camera
        .region()
        .expect("Failed to read camera region after configuration.");
    let payload = camera
        .payload()
        .expect("failed to read payload after configuration");
    println!(
        "Configured camera {}: width={} height={} payload={} exposure_us={} frame_rate_hz={}",
        config.camera_id, width, height, payload, config.exposure_time_us, config.frame_rate_hz
    );

    // Start Aravis camera aquisition.
    camera
        .start_acquisition()
        .expect("Failed to start camera acquisition.");

    // Keep track of start time and the number of saved frames.
    let start_time = Instant::now();
    let mut frames_saved = 0usize;

    // Used to only print timeouts after first buffer arrives.
    let mut first_buffer_arrived = false;

    // Main recording loop.
    loop {
        // Stop streaming if a maximum number of frames was configured and
        // the camera has recorded that many frames.
        if let Some(limit) = max_frames {
            if frames_saved >= limit {
                break;
            }
        }

        // Stop streaming if a maximum duration was configured and the
        // camera has recorded for that amount of time.
        if let Some(seconds) = max_duration {
            if start_time.elapsed() >= Duration::from_secs_f64(seconds) {
                break;
            }
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
                println!(
                    "Frame {} received at {:.2}s",
                    frames_saved,
                    elapsed_since_start.as_secs_f64()
                );

                let data = copy_buffer_bytes(&buffer);

                if data.is_empty() {
                    eprintln!("Empty buffer from camera {}.", config.camera_id);
                    stream.push_buffer(buffer);
                    continue;
                }

                // Store metadata.
                let metadata = FrameMetadata {
                    camera_id: config.camera_id.clone(),
                    frame_index: frames_saved,
                    width,
                    height,
                    payload_bytes: data.len(),
                    system_timestamp_ns: buffer.system_timestamp(),
                    buffer_timestamp_ns: buffer.timestamp(),
                    frame_id: buffer.frame_id(),
                    exposure_time_us: config.exposure_time_us,
                    frame_rate_hz: config.frame_rate_hz,
                };

                // Write the frame's raw bytes and metadata to file.
                write_frame_files(
                    &output_camera_dir,
                    &config.camera_id,
                    frames_saved,
                    &data,
                    &metadata,
                );

                frames_saved += 1;
            }
            status => {
                eprintln!(
                    "ERROR: Camera {} returned non-success buffer status: {:?}",
                    config.camera_id, status
                );
            }
        }

        stream.push_buffer(buffer);
    }

    let _ = camera.stop_acquisition();
    println!(
        "Finished recording from camera {}. Saved {} frame(s) into {}",
        config.camera_id,
        frames_saved,
        output_camera_dir.display()
    );
}
