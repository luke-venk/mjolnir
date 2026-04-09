use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::camera::CameraIngestConfig;
use super::compression::{
    COMPRESSED_FRAME_EXTENSION, compress_mono8_frame, recover_compressed_dir_to_pngs,
};
use super::writer::{FrameMetadata, ensure_dir, sanitize_path_name, write_frame_files};
use crate::camera::aravis_utils::{
    configure_camera, copy_buffer_bytes, create_camera, create_stream_and_allocate_buffers,
};

use aravis::{BufferStatus, CameraExt, StreamExt};

/// Records stream from a single camera.
pub fn record_from_one_camera(
    config: &CameraIngestConfig,
    output_base_dir: &PathBuf,
    recover_base_dir: Option<&PathBuf>,
    max_frames: Option<usize>,
    max_duration: Option<f64>,
) {
    println!("Beginning recording for camera {}.",config.camera_id);

    // Ensures that output directory for this specific camera exists.
    let output_camera_dir = output_base_dir.join(sanitize_path_name(&config.camera_id));
    ensure_dir(&output_camera_dir);
    let recover_camera_dir = recover_base_dir.map(|base| base.join(sanitize_path_name(&config.camera_id)));
    if let Some(recover_camera_dir) = recover_camera_dir.as_ref() {
        ensure_dir(recover_camera_dir);
    }

    // Create Aravis camera, apply configuration, start stream, and queue buffers.
    let camera = match create_camera(&config.camera_id) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            return;
        },
    };
    configure_camera(&camera, &config);
    let stream = create_stream_and_allocate_buffers(&camera, config.num_buffers);

    // For frame metadata.
    let (width, height) = config.resolution.dimensions();

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

                // Write either the raw frame bytes or their compressed form to disk.
                if config.compress {
                    let compressed = compress_mono8_frame(&data)
                        .unwrap_or_else(|err| panic!("failed to compress frame {}: {err:#}", frames_saved));
                    let _written = write_frame_files(
                        &output_camera_dir,
                        &config.camera_id,
                        frames_saved,
                        &compressed,
                        &metadata,
                        COMPRESSED_FRAME_EXTENSION,
                    );
                } else {
                    let _written = write_frame_files(
                        &output_camera_dir,
                        &config.camera_id,
                        frames_saved,
                        &data,
                        &metadata,
                        "raw",
                    );
                }

                frames_saved += 1;

                if frames_saved % 10 == 0 {
                    println!(
                        "Camera {}: saved {} frame(s)",
                        config.camera_id, frames_saved
                    );
                }
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
    if config.compress {
        if let Some(recover_camera_dir) = recover_camera_dir.as_ref() {
            let recovered = recover_compressed_dir_to_pngs(&output_camera_dir, recover_camera_dir)
                .unwrap_or_else(|err| panic!("{err:#}"));
            println!(
                "Recovered {} PNG frame(s) for camera {} into {}",
                recovered,
                config.camera_id,
                recover_camera_dir.display()
            );
        }
    }
    println!(
        "Finished recording from camera {}. Saved {} frame(s) into {}",
        config.camera_id,
        frames_saved,
        output_camera_dir.display()
    );
}
