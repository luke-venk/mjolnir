/// Capture thread that runs alongside UI thread that pulls frames from
/// Aravis and places them into shared buffer for the UI thread to read.
use std::sync::{Arc, Mutex};

use super::FrameData;
use crate::camera::aravis_utils::{
    configure_camera, copy_buffer_bytes, create_camera, create_stream_and_allocate_buffers,
};
use crate::camera::CameraIngestConfig;
use aravis::{BufferStatus, CameraExt, StreamExt};

pub fn run_capture_thread(
    config: Arc<Mutex<CameraIngestConfig>>,
    latest_frame: Arc<Mutex<Option<FrameData>>>,
) {
    let settings = config.lock().expect("Error: Failed to unlock camera settings");
    
    println!("Capture thread started for camera: {}", settings.camera_id);

    // Create Aravis camera, apply configuration, start stream, and queue buffers.
    let camera = match create_camera(&settings.camera_id) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            return;
        },
    };
    configure_camera(&camera, &settings.clone());
    let stream = create_stream_and_allocate_buffers(&camera, settings.num_buffers);

    // Start Aravis camera aquisition.
    camera
        .start_acquisition()
        .expect("Failed to start camera acquisition.");

    println!("Acquisition started.");

    // Live streaming loop.
    loop {
        // Load camera buffer.
        // Block current thread until frame buffer delivered or the timeout elapses.
        let buffer = match stream.timeout_pop_buffer(settings.timeout_ms * 1000) {
            Some(b) => b,
            None => {
                eprintln!("Error: timeout_pop_buffer() timed out during streaming!");
                continue
            },
        };

        // If loading the buffer worked, copy the frame buffer into raw bytes.
        match buffer.status() {
            BufferStatus::Success => {
                let data = copy_buffer_bytes(&buffer);
                if !data.is_empty() {
                    let frame = FrameData {
                        pixels: data,
                        width: settings.resolution.dimensions().0 as u32,
                        height: settings.resolution.dimensions().1 as u32,
                        received_at_ns: buffer.timestamp(),
                    };

                    // If successfully acquired mutex's lock, update the latest frame safely
                    // through the mutex.
                    if let Ok(mut lock) = latest_frame.lock() {
                        *lock = Some(frame);
                    } else {
                        eprintln!("ERROR: Lock not acquired.");
                    }
                }
            },
            _ => {
                eprintln!("Error: BufferStatus was not a success in streaming.");
            },
        }

        stream.push_buffer(buffer);
    }

    // TODO: clean shutdown stop acquisition
}
