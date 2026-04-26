use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use aravis::{BufferStatus, CameraExt, StreamExt};
use crossbeam::channel::{Receiver, RecvTimeoutError, Sender};
use crate::camera::CameraIngestConfig;
use crate::camera::aravis_utils::{
    configure_camera, create_camera, create_stream_and_allocate_buffers, initialize_aravis,
};
use crate::camera::record::writer::Frame as RecordedFrame;
use crate::camera_ingest::camera_ingest_helpers::{buffer_to_frame, forward_recorded_frame};
use crate::schemas::Frame as PipelineFrame;

// Ingests frames straight from the cameras using the GigEVision API and sends them into the pipeline 
// if we wanted it straight from recording (if we wanted to send frames straight from recording)
pub fn ingest_frames(tx: Sender<PipelineFrame>, config: CameraIngestConfig) {
    config
        .validate()
        .unwrap_or_else(|err| panic!("invalid camera ingest config: {err}"));

    let mut frames_sent = 0usize;

    initialize_aravis();
    let camera = create_camera(&config.camera_id).unwrap_or_else(|err| panic!("{err}"));
    configure_camera(&camera, &config, None, None);
    let stream = create_stream_and_allocate_buffers(&camera, config.num_buffers);

    camera
        .start_acquisition()
        .expect("Failed to start acquisition.");

    loop {
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
                println!("camera_ingest: sent live frame {} into pipeline", frames_sent);

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

// Receives recorded frames, sorts them by camera, converts them, and forwards
// them into already-running left/right pipelines.
pub fn run_recording_ingest(
    frame_rx: Receiver<RecordedFrame>,
    left_camera_id: String,
    right_camera_id: String,
    left_tx: Sender<PipelineFrame>,
    right_tx: Sender<PipelineFrame>,
    shutdown: Arc<AtomicBool>,
) {
    loop {
        if shutdown.load(Ordering::SeqCst) {
            break;
        }

        let recorded_frame = match frame_rx.recv_timeout(Duration::from_millis(100)) {
            Ok(frame) => frame,
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => break,
        };

        if !forward_recorded_frame(
            recorded_frame,
            &left_camera_id,
            &right_camera_id,
            &left_tx,
            &right_tx,
        ) {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::thread;

    use crossbeam::channel::bounded;

    use super::run_recording_ingest;
    use crate::camera::record::writer::{Frame as RecordedFrame, Metadata};
    use crate::schemas::Frame as PipelineFrame;

    #[test]
    fn recording_ingest_routes_frames_by_camera() {
        let (recorded_tx, recorded_rx) = bounded(4);
        let (left_tx, left_rx) = bounded(4);
        let (right_tx, right_rx) = bounded(4);
        let shutdown = Arc::new(AtomicBool::new(false));

        recorded_tx
            .send(RecordedFrame {
                output_camera_dir: Some(PathBuf::new()),
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

        recorded_tx
            .send(RecordedFrame {
                output_camera_dir: Some(PathBuf::new()),
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

        drop(recorded_tx);

        let handle = thread::spawn(move || {
            run_recording_ingest(
                recorded_rx,
                "left-camera".to_string(),
                "right-camera".to_string(),
                left_tx,
                right_tx,
                shutdown,
            );
        });

        handle.join().expect("recording ingest should complete");

        let left_frames: Vec<_> = left_rx.try_iter().collect();
        let right_frames: Vec<_> = right_rx.try_iter().collect();

        assert_eq!(left_frames.len(), 1);
        assert_eq!(left_frames[0].data(), &[1, 2, 3, 4]);
        assert_eq!(left_frames[0].context().timestamp(), 11);
        assert_eq!(right_frames.len(), 1);
        assert_eq!(right_frames[0].data(), &[5, 6, 7, 8]);
        assert_eq!(right_frames[0].context().timestamp(), 22);
    }

    #[test]
    fn recording_ingest_stops_when_shutdown_is_requested() {
        let (_recorded_tx, recorded_rx) = bounded(4);
        let (left_tx, _left_rx) = bounded::<PipelineFrame>(4);
        let (right_tx, _right_rx) = bounded::<PipelineFrame>(4);
        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_clone = Arc::clone(&shutdown);

        let handle = thread::spawn(move || {
            run_recording_ingest(
                recorded_rx,
                "left-camera".to_string(),
                "right-camera".to_string(),
                left_tx,
                right_tx,
                shutdown_clone,
            );
        });

        shutdown.store(true, Ordering::SeqCst);

        handle.join().expect("recording ingest should stop on shutdown");
    }
}
