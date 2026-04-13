use std::thread;
use std::time::{Duration, Instant};

use aravis::{BufferStatus, CameraExt, StreamExt};
use crossbeam::channel::Sender;

use crate::camera_ingest::camera_ingest_helpers::{
    buffer_to_frame, configure_camera, create_stream_and_queue_buffers, initialize_aravis,
    open_camera,
};
use crate::schemas::camera_ingest_config::CameraIngestConfig;
use crate::schemas::{Context, Frame};

// Ingests frames from the cameras using the GigEVision API, and enqueues
// the frames into the camera ingest sender for the camera's pipeline to
// begin processing.
pub fn ingest_frames(tx: Sender<Frame>, config: CameraIngestConfig) {
    let config = config.validate().expect("invalid camera ingest config");
    let start_time = Instant::now();
    let mut frames_sent = 0usize;

    if config.use_fake_interface {
        loop {
            if let Some(limit) = config.max_frames {
                if frames_sent >= limit {
                    break;
                }
            }

            if let Some(seconds) = config.max_duration_s {
                if start_time.elapsed() >= Duration::from_secs_f64(seconds) {
                    break;
                }
            }

            let data = vec![1, 2, 3, 4];
            let context = Context::new(1234);
            if tx.send(Frame::new(data, context)).is_err() {
                break;
            }
            frames_sent += 1;
            thread::sleep(Duration::from_millis(10));
        }

        return;
    }

    initialize_aravis();
    let camera = open_camera(&config);
    configure_camera(&camera, &config);
    let stream = create_stream_and_queue_buffers(&camera, config.num_buffers);

    camera
        .start_acquisition()
        .expect("Failed to start acquisition.");

    loop {
        if let Some(limit) = config.max_frames {
            if frames_sent >= limit {
                break;
            }
        }

        if let Some(seconds) = config.max_duration_s {
            if start_time.elapsed() >= Duration::from_secs_f64(seconds) {
                break;
            }
        }

        let buffer = match stream.timeout_pop_buffer(config.timeout_ms * 1000) {
            Some(buffer) => buffer,
            None => continue,
        };

        match buffer.status() {
            BufferStatus::Success => {
                let frame = buffer_to_frame(&buffer);

                if tx.send(frame).is_err() {
                    stream.push_buffer(buffer);
                    break;
                }

                frames_sent += 1;
            }
            status => {
                eprintln!("Buffer status: {:?}", status);
            }
        }

        stream.push_buffer(buffer);
    }

    let _ = camera.stop_acquisition();
}

#[cfg(test)]
mod tests {
    use crossbeam::channel::bounded;

    use super::ingest_frames;
    use crate::schemas::camera_ingest_config::{CameraIngestConfig, Resolution};

    #[test]
    fn fake_ingest_stops_after_max_frames() {
        let (tx, rx) = bounded(4);
        let config = CameraIngestConfig {
            device_id: String::new(),
            exposure_time_us: 1000.0,
            frame_rate_hz: 30.0,
            resolution: Resolution::UHD4K,
            aperture: None,
            enable_ptp: false,
            use_fake_interface: true,
            num_buffers: 2,
            timeout_ms: 100,
            max_frames: Some(2),
            max_duration_s: None,
        };

        ingest_frames(tx, config);

        let frames: Vec<_> = rx.try_iter().collect();
        assert_eq!(frames.len(), 2);
    }
}
