/// Capture thread that runs alongside UI thread that pulls frames from
/// Aravis and places them into shared buffer for the UI thread to read.
use std::sync::{Arc, Mutex};
use std::time::Instant;

use super::FrameData;
use crate::camera::aravis_utils::{
    configure_camera, copy_buffer_bytes, create_camera, create_stream_and_allocate_buffers,
};
use crate::camera::CameraIngestConfig;
use crate::camera::stream::CameraSettings;
use aravis::{BufferStatus, CameraExt, StreamExt};

pub fn run_capture_thread(
    config: CameraIngestConfig,
    latest_frame: Arc<Mutex<Option<FrameData>>>,
    camera_settings: Arc<Mutex<CameraSettings>>,
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

    // Start Aravis camera aquisition.
    camera
        .start_acquisition()
        .expect("Failed to start camera acquisition.");

    println!("Acquisition started.");

    let mut acquisition_start = Instant::now();

    // Live streaming loop.
    loop {
        // Debug statement to check if slider changed values here.
        // println!("Exposure time (µs): {}", camera_settings.lock().unwrap().exposure_us);
        // println!("Frame rate (Hz): {}", camera_settings.lock().unwrap().frame_rate_hz);

        // Load camera buffer.
        // Block current thread until frame buffer delivered or the timeout elapses.
        let buffer = match stream.timeout_pop_buffer(config.timeout_ms * 1000) {
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
                        width: config.resolution.dimensions().0 as u32,
                        height: config.resolution.dimensions().1 as u32,
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

        let now = Instant::now();
        println!("Instataneous frame rate = {}", 1000.0 / now.duration_since(acquisition_start).as_millis() as f64);

        acquisition_start = now;

    }
}
