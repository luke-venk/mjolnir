use std::thread;
use std::time::{Duration, Instant};

use aravis::{BufferStatus, CameraExt, StreamExt};
use crossbeam::channel::{Receiver, Sender, bounded};

use crate::camera::aravis_utils::{
    configure_camera, create_camera, create_stream_and_allocate_buffers, initialize_aravis,
};
use crate::camera::{CameraIngestConfig, Resolution};
use crate::camera::record::writer::Frame as RecordedFrame;
use crate::camera_ingest::camera_ingest_helpers::{
    buffer_to_frame, reached_duration_limit, reached_frame_limit, recorded_frame_to_frame,
};
use crate::pipeline::Pipeline;
use crate::schemas::{CameraId, Context, Frame};

// Ingests frames from the cameras using the GigEVision API and sends them into the pipeline.
pub fn ingest_frames(tx: Sender<Frame>, config: CameraIngestConfig) {
    config
        .validate()
        .unwrap_or_else(|err| panic!("invalid camera ingest config: {err}"));

    let start_time = Instant::now();
    let mut frames_sent = 0usize;

    if config.use_fake_interface {
        loop {
            if reached_frame_limit(frames_sent, config.max_frames) {
                break;
            }
            if reached_duration_limit(start_time, config.max_duration_s) {
                break;
            }

            let frame = Frame::new(vec![1, 2, 3, 4], Context::new(frames_sent as u64));
            if tx.send(frame).is_err() {
                break;
            }

            frames_sent += 1;
            thread::sleep(Duration::from_millis(10));
        }

        return;
    }

    initialize_aravis();
    let camera = create_camera(&config.camera_id)
        .unwrap_or_else(|err| panic!("{err}"));
    configure_camera(&camera, &config);
    let stream = create_stream_and_allocate_buffers(&camera, config.num_buffers);

    camera
        .start_acquisition()
        .expect("Failed to start acquisition.");

    loop {
        if reached_frame_limit(frames_sent, config.max_frames) {
            break;
        }
        if reached_duration_limit(start_time, config.max_duration_s) {
            break;
        }

        let buffer = match stream.timeout_pop_buffer(config.timeout_ms * 1000) {
            Some(buffer) => buffer,
            None => continue,
        };

        match buffer.status() {
            BufferStatus::Success => {
                let frame = buffer_to_frame(&buffer);
                if frame.data().is_empty() {
                    stream.push_buffer(buffer);
                    continue;
                }

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

// Receives recorded frames, sorts them by camera, converts them, and forwards them into the pipelines.
pub fn run_recording_ingest(
    frame_rx: Receiver<RecordedFrame>,
    left_camera_id: String,
    right_camera_id: String,
    capacity_per_channel: usize,
) {
    let (left_tx, left_rx) = bounded::<Frame>(capacity_per_channel);
    let (right_tx, right_rx) = bounded::<Frame>(capacity_per_channel);

    let left_pipeline = Pipeline::from_receiver(CameraId::FieldLeft, left_rx, capacity_per_channel);
    let right_pipeline = Pipeline::from_receiver(CameraId::FieldRight, right_rx, capacity_per_channel);

    for recorded_frame in frame_rx {
        let source_camera_id = recorded_frame.metadata.camera_id.clone();
        let frame = recorded_frame_to_frame(recorded_frame);

        let send_result = if source_camera_id == left_camera_id {
            left_tx.send(frame)
        } else if source_camera_id == right_camera_id {
            right_tx.send(frame)
        } else {
            eprintln!("Received frame for unexpected camera {}.", source_camera_id);
            continue;
        };

        if send_result.is_err() {
            break;
        }
    }

    drop(left_tx);
    drop(right_tx);
    left_pipeline.stop();
    right_pipeline.stop();
}

// Starts one camera-ingest thread and one pipeline stage graph for a camera config.
pub fn start_camera_pipeline(
    camera_id: CameraId,
    config: CameraIngestConfig,
    capacity_per_channel: usize,
) -> (thread::JoinHandle<()>, Pipeline) {
    let (tx_ingest, rx_stage1) = bounded::<Frame>(capacity_per_channel);
    let ingest_handle = thread::spawn(move || {
        ingest_frames(tx_ingest, config);
    });
    let pipeline = Pipeline::from_receiver(camera_id, rx_stage1, capacity_per_channel);

    (ingest_handle, pipeline)
}

// Starts one default fake camera pipeline so existing startup flows still work.
pub fn start_default_camera_pipeline(
    camera_id: CameraId,
    capacity_per_channel: usize,
) -> (thread::JoinHandle<()>, Pipeline) {
    start_camera_pipeline(
        camera_id,
        default_ingest_config(camera_id),
        capacity_per_channel,
    )
}

fn default_ingest_config(camera_id: CameraId) -> CameraIngestConfig {
    CameraIngestConfig {
        camera_id: format!("{camera_id:?}"),
        exposure_time_us: 10000.0,
        frame_rate_hz: 30.0,
        resolution: Resolution::UHD4K,
        enable_ptp: false,
        num_buffers: 8,
        timeout_ms: 5000,
        use_fake_interface: true,
        max_frames: Some(1),
        max_duration_s: None,
        restart_requested: false,
    }
}


#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::thread;

    use crossbeam::channel::bounded;

    use super::{
        ingest_frames, run_recording_ingest, start_camera_pipeline,
    };
    use crate::camera::Resolution;
    use crate::camera::CameraIngestConfig;
    use crate::camera::record::writer::{Frame as RecordedFrame, Metadata};
    use crate::schemas::CameraId;

    #[test]
    fn fake_ingest_stops_after_max_frames() {
        let (tx, rx) = bounded(4);
        let config = CameraIngestConfig {
            camera_id: String::new(),
            exposure_time_us: 1000.0,
            frame_rate_hz: 30.0,
            resolution: Resolution::UHD4K,
            enable_ptp: false,
            num_buffers: 2,
            timeout_ms: 100,
            use_fake_interface: true,
            max_frames: Some(2),
            max_duration_s: None,
            restart_requested: false,
        };

        ingest_frames(tx, config);

        let frames: Vec<_> = rx.try_iter().collect();
        assert_eq!(frames.len(), 2);
    }

    #[test]
    fn recording_ingest_returns_after_input_closes() {
        let (tx, rx) = bounded(4);

        tx.send(RecordedFrame {
            output_camera_dir: PathBuf::new(),
            frame_index: 0,
            bytes: vec![1, 2, 3, 4],
            metadata: Metadata {
                camera_id: "left-camera".to_string(),
                frame_index: 0,
                width: 4,
                height: 1,
                payload_bytes: 4,
                system_timestamp_ns: 11,
                buffer_timestamp_ns: 0,
                frame_id: 0,
            },
        })
        .unwrap();

        tx.send(RecordedFrame {
            output_camera_dir: PathBuf::new(),
            frame_index: 0,
            bytes: vec![5, 6, 7, 8],
            metadata: Metadata {
                camera_id: "right-camera".to_string(),
                frame_index: 0,
                width: 4,
                height: 1,
                payload_bytes: 4,
                system_timestamp_ns: 22,
                buffer_timestamp_ns: 0,
                frame_id: 0,
            },
        })
        .unwrap();

        drop(tx);

        let handle = thread::spawn(move || {
            run_recording_ingest(rx, "left-camera".to_string(), "right-camera".to_string(), 4);
        });

        handle.join().expect("recording ingest should complete");
    }

    #[test]
    fn start_camera_pipeline_completes_with_fake_config() {
        let config = CameraIngestConfig {
            camera_id: String::new(),
            exposure_time_us: 1000.0,
            frame_rate_hz: 30.0,
            resolution: Resolution::UHD4K,
            enable_ptp: false,
            num_buffers: 2,
            timeout_ms: 100,
            use_fake_interface: true,
            max_frames: Some(1),
            max_duration_s: None,
            restart_requested: false,
        };

        let (ingest_handle, pipeline) = start_camera_pipeline(CameraId::FieldLeft, config, 4);
        ingest_handle
            .join()
            .expect("camera ingest thread should complete");
        pipeline.stop();
    }
}
