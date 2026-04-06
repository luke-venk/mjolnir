/// Capture thread that runs alongside UI thread that pulls frames from
/// Aravis and places them into shared buffer for the UI thread to read.
use std::sync::{Arc, Mutex};
use std::time::Instant;

use super::FrameData;
use crate::camera::aravis_utils::{
    configure_camera, copy_buffer_bytes, create_camera, create_stream_and_allocate_buffers,
};
use crate::camera::CameraIngestConfig;
use aravis::{BufferStatus, CameraExt, StreamExt};

pub fn run_capture_thread(
    config: CameraIngestConfig,
    latest_frame: Arc<Mutex<Option<FrameData>>>,
) {
    println!("Capture thread started for camera: {}", config.camera_id);

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

    // Data for frame metadata.
    let (_, _, width, height) = camera
        .region()
        .expect("Failed to read camera region.");

    // Start Aravis camera aquisition.
    camera
        .start_acquisition()
        .expect("Failed to start camera acquisition.");

    println!("Acquisition started.");

    // Live streaming loop.
    loop {
        // Load camera buffer.
        // Block current thread until frame buffer delivered or the timeout elapses.
        let buffer = match stream.timeout_pop_buffer(config.timeout_ms * 1000) {
            Some(b) => b,
            None => continue,
        };

        // If loading the buffer worked, copy the frame buffer into raw bytes.
        match buffer.status() {
            BufferStatus::Success => {
                let data = copy_buffer_bytes(&buffer);
                if !data.is_empty() {
                    let frame = FrameData {
                        pixels: data,
                        width: width as u32,
                        height: height as u32,
                        received_at: Instant::now(),
                    };

                    // If successfully acquired mutex's lock, update the latest frame safely
                    // through the mutex.
                    if let Ok(mut lock) = latest_frame.lock() {
                        *lock = Some(frame);
                    }
                }
            },
            _ => {},
        }

        stream.push_buffer(buffer);
    }
}
