use crate::frame::FrameData;
use aravis::{BufferStatus, CameraExt, StreamExt};
use backend_lib::camera::aravis_utils::{
    configure_camera, copy_buffer_bytes, create_camera, create_stream_and_allocate_buffers,
};
use backend_lib::camera::CameraIngestConfig;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// Capture thread that runs alongside UI thread that pulls frames from
/// Aravis and places them into shared buffer for the UI thread to read.

/// Live stream from the camera.
pub fn run_capture_thread(
    config: Arc<Mutex<CameraIngestConfig>>,
    frame_tx: crossbeam::channel::Sender<FrameData>,
    shutdown: Arc<AtomicBool>,
) {
    // Lock settings mutex briefly to read values.
    let (camera_id, num_buffers, timeout_ms, resolution) = match config.lock() {
        Ok(settings) => (
            settings.camera_id.clone(),
            settings.num_buffers,
            settings.timeout_ms,
            settings.resolution,
        ),
        Err(_) => {
            eprintln!("Error: Failed to lock camera settings mutex.");
            return;
        }
    };

    println!("Capture thread started for camera: {}", camera_id);

    // Create Aravis camera, apply configuration, start stream, and queue buffers.
    let camera = match create_camera(&camera_id) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            return;
        }
    };

    // Lock settings mutex briefly to configure camera through Aravis.
    {
        let settings = config
            .lock()
            .expect("Error: Failed to lock camera settings mutex.");
        configure_camera(&camera, &settings, None, None);
    }

    let mut stream = create_stream_and_allocate_buffers(&camera, num_buffers);

    // Start Aravis camera aquisition.
    camera
        .start_acquisition()
        .expect("Failed to start camera acquisition.");

    println!("Acquisition started.");

    // Live streaming loop.
    loop {
        // Check shutdown flag.
        if shutdown.load(Ordering::SeqCst) {
            println!("Shutting down capture for camera {}.", camera_id);
            break;
        }

        // Briefly lock camera settings mutex to check whether a restart was requested or not.
        let restart = {
            let mut settings = config
                .lock()
                .expect("Error: Failed to lock camera settings mutex.");
            if settings.restart_requested {
                settings.restart_requested = false;
                true
            } else {
                false
            }
        };

        // If a restart was requested through the button on the screen, restart
        // the acquisition with the new specifications.
        if restart {
            println!("Restart requested.");
            camera
                .stop_acquisition()
                .expect("Error: Failed to stop camera acquisition.");
            {
                let settings = config
                    .lock()
                    .expect("Error: Failed to lock camera settings mutex.");
                configure_camera(&camera, &settings, None, None);
            }
            stream = create_stream_and_allocate_buffers(&camera, num_buffers);
            camera
                .start_acquisition()
                .expect("Error: Failed to restart camera acquisition.");
        }

        // Load camera buffer.
        // Block current thread until frame buffer delivered or the timeout elapses.
        let buffer = match stream.timeout_pop_buffer(timeout_ms * 1000) {
            Some(b) => b,
            None => {
                eprintln!("Error: timeout_pop_buffer() timed out during streaming!");
                continue;
            }
        };

        // If loading the buffer worked, copy the frame buffer into raw bytes.
        match buffer.status() {
            BufferStatus::Success => {
                let data = copy_buffer_bytes(&buffer);
                let received_at_ns = buffer.timestamp();
                stream.push_buffer(buffer);

                if !data.is_empty() {
                    let frame = FrameData {
                        pixels: data,
                        width: resolution.dimensions().0 as u32,
                        height: resolution.dimensions().1 as u32,
                        received_at_ns,
                    };

                    // Send this captured frame to the UI thread.
                    frame_tx.send(frame).expect(
                        "Error: Failed to send frame from streaming capture thread to UI thread.",
                    );
                }
            }
            _ => {
                eprintln!("Error: BufferStatus was not a success in streaming.");
            }
        }
    }
}
