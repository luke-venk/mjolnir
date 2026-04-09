use std::path::PathBuf;
use std::time::{Duration, Instant};
use crate::camera::CameraIngestConfig;
use super::writer::{Frame, Metadata, ensure_dir, sanitize_path_name};
use crate::camera::aravis_utils::{
    configure_camera, copy_buffer_bytes, create_camera, create_stream_and_allocate_buffers,
};

use aravis::{BufferStatus, CameraExt, StreamExt};

/// Records stream from a single camera.
pub fn run_capture_thread(
    output_base_dir: PathBuf,
    config: &CameraIngestConfig,
    frame_tx: crossbeam::channel::Sender<Frame>,
    max_frames: Option<usize>,
    max_duration: Option<f64>,
) {
    println!("Beginning recording for camera {}.",config.camera_id);
    
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
                let elapsed_since_start = start_time.elapsed();
                println!(
                    "Frame {} received at {:.2}s for {}.",
                    frames_saved,
                    elapsed_since_start.as_secs_f64(),
                    camera_id,
                );

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
                frame_tx.send(frame).expect("Error: Failed to send frame from recording capture thread to write thread.");

                frames_saved += 1;
            }
            status => {
                eprintln!(
                    "ERROR: Camera {} returned non-success buffer status: {:?}",
                    config.camera_id, status
                );
            }
        }
    }

    let _ = camera.stop_acquisition();
    println!(
        "Finished recording from camera {}. Saved {} frame(s) into {}",
        config.camera_id,
        frames_saved,
        output_camera_dir.display()
    );
}
