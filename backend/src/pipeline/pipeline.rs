use super::PipelineStage;
use crate::camera::CameraIngestConfig;
use crate::camera::record::{
    CaptureStopConditions, DualCameraCapture, start_dual_camera_capture,
};
use crate::camera_ingest::{ingest_frames, replay_recorded_session, run_recording_ingest};
use crate::computer_vision::{
    contour, forward_downsampled_copy, intensity_normalization, mog2, undistortion,
};
use crate::schemas::{CameraId, Frame as PipelineFrame};
use crossbeam::channel::{Receiver, bounded};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::thread::{self, JoinHandle};

pub struct Pipeline {
    _camera_id: CameraId,
    handles: Vec<JoinHandle<()>>,
}

#[allow(dead_code)]
impl Pipeline {
    // Builds one pipeline stage graph around an incoming frame receiver.
    pub fn new(
        camera_id: CameraId,
        rx_stage1: Receiver<PipelineFrame>,
        capacity_per_channel: usize,
    ) -> Self {
        let (tx_stage1, rx_stage2) = bounded::<PipelineFrame>(capacity_per_channel);
        let (tx_stage2, rx_stage3) = bounded::<PipelineFrame>(capacity_per_channel);
        let (tx_stage3, rx_stage4) = bounded::<PipelineFrame>(capacity_per_channel);
        let (tx_stage4, rx_stage5) = bounded::<PipelineFrame>(capacity_per_channel);
        let (tx_stage5, rx_output) = bounded::<PipelineFrame>(capacity_per_channel);

        let handle_stage1 = PipelineStage::new(rx_stage1, tx_stage1, undistortion).spawn();
        let handle_stage2 =
            PipelineStage::new(rx_stage2, tx_stage2, intensity_normalization).spawn();
        let handle_stage3 =
            PipelineStage::new(rx_stage3, tx_stage3, forward_downsampled_copy).spawn();
        let handle_stage4 = PipelineStage::new(rx_stage4, tx_stage4, mog2).spawn();
        let handle_stage5 = PipelineStage::new(rx_stage5, tx_stage5, contour).spawn();

        //Checking if frames made it through full pipeline 
        let handle_output = thread::spawn(move || {
            for frame in rx_output.iter() {
                println!(
                    "pipeline: {:?} produced output frame at timestamp {}",
                    camera_id,
                    frame.context().timestamp()
                );
                // TODO: forward results to output.
            }
        });

        Self {
            _camera_id: camera_id,
            handles: vec![
                handle_stage1,
                handle_stage2,
                handle_stage3,
                handle_stage4,
                handle_stage5,
                handle_output,
            ],
        }
    }

    // Safely brings down all pipeline stage threads when system stops,
    // and blocks the main thread until these come down.
    pub fn stop(self) {
        for handle in self.handles {
            let _ = handle.join();
        }
    }
}

// Starts one camera-ingest thread and one pipeline stage graph for a camera config.
pub fn start_camera_pipeline(
    camera_id: CameraId,
    config: CameraIngestConfig,
    capacity_per_channel: usize,
) -> (JoinHandle<()>, Pipeline) {
    let (tx_ingest, rx_stage1) = bounded::<PipelineFrame>(capacity_per_channel);
    let ingest_handle = thread::spawn(move || {
        ingest_frames(tx_ingest, config);
    });
    let pipeline = Pipeline::new(camera_id, rx_stage1, capacity_per_channel);

    (ingest_handle, pipeline)
}

// Starts one replay thread and one left/right pipeline pair for recorded footage.
pub fn start_recorded_footage_pipelines(
    footage_dir: PathBuf,
    capacity_per_channel: usize,
) -> (JoinHandle<()>, Pipeline, Pipeline) {
    let (left_tx, left_rx) = bounded::<PipelineFrame>(capacity_per_channel);
    let (right_tx, right_rx) = bounded::<PipelineFrame>(capacity_per_channel);
    let replay_handle = thread::spawn(move || {
        replay_recorded_session(footage_dir, left_tx, right_tx);
    });
    let left_pipeline = Pipeline::new(CameraId::FieldLeft, left_rx, capacity_per_channel);
    let right_pipeline = Pipeline::new(CameraId::FieldRight, right_rx, capacity_per_channel);

    (replay_handle, left_pipeline, right_pipeline)
}

pub fn start_recording_camera_pipelines(
    interface: Option<&str>,
    left_config: CameraIngestConfig,
    right_config: CameraIngestConfig,
    capacity_per_channel: usize,
) -> (DualCameraCapture, JoinHandle<()>, Pipeline, Pipeline) {
    let left_camera_id = left_config.camera_id.clone();
    let right_camera_id = right_config.camera_id.clone();
    let shutdown = Arc::new(AtomicBool::new(false));
    let (frame_rx, capture_runtime) = start_dual_camera_capture(
        None,
        interface,
        left_config,
        right_config,
        CaptureStopConditions::default(),
        Arc::clone(&shutdown),
        capacity_per_channel,
    );
    let (left_tx, left_rx) = bounded::<PipelineFrame>(capacity_per_channel);
    let (right_tx, right_rx) = bounded::<PipelineFrame>(capacity_per_channel);
    let ingest_handle = thread::spawn(move || {
        run_recording_ingest(
            frame_rx,
            left_camera_id,
            right_camera_id,
            left_tx,
            right_tx,
            shutdown,
        );
    });
    let left_pipeline = Pipeline::new(CameraId::FieldLeft, left_rx, capacity_per_channel);
    let right_pipeline = Pipeline::new(CameraId::FieldRight, right_rx, capacity_per_channel);

    (capture_runtime, ingest_handle, left_pipeline, right_pipeline)
}



#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;
    use std::path::PathBuf;
    use std::thread;
    use std::time::Duration;
    use std::time::{SystemTime, UNIX_EPOCH};

    use crossbeam::channel::bounded;

    use super::Pipeline;
    use crate::camera::record::writer::{Metadata, SessionManifest, SESSION_MANIFEST_FILE_NAME};
    use crate::camera_ingest::replay_recorded_session;
    use crate::computer_vision::{
        contour, forward_downsampled_copy, intensity_normalization, mog2, undistortion,
    };
    use crate::pipeline::PipelineStage;
    use crate::schemas::{CameraId, Context, Frame as PipelineFrame};

    #[test]
    fn pipeline_new_starts_and_stops_after_input_channel_closes() {
        let (tx, rx) = bounded::<PipelineFrame>(2);
        let pipeline = Pipeline::new(CameraId::FieldLeft, rx, 2);

        tx.send(PipelineFrame::new(vec![1, 2, 3, 4], Context::new(1)))
            .expect("send pipeline frame");
        drop(tx);

        pipeline.stop();
    }

    #[test]
    fn recorded_session_replay_crosses_full_left_and_right_pipeline_graphs() {
        let temp_dir = env::temp_dir().join(format!(
            "mjolnir_pipeline_replay_test_{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time should be after epoch")
                .as_nanos()
        ));
        let left_dir = temp_dir.join("camera-b");
        let right_dir = temp_dir.join("camera-a");
        fs::create_dir_all(&left_dir).expect("create left test dir");
        fs::create_dir_all(&right_dir).expect("create right test dir");
        fs::write(
            temp_dir.join(SESSION_MANIFEST_FILE_NAME),
            serde_json::to_vec_pretty(&SessionManifest {
                left_camera_id: "camera-b".to_string(),
                right_camera_id: "camera-a".to_string(),
            })
            .expect("serialize session manifest"),
        )
        .expect("write session manifest");

        write_test_recorded_frame(
            &left_dir,
            &Metadata {
                camera_id: "camera-b".to_string(),
                frame_index: 0,
                width: 4,
                height: 1,
                payload_bytes: 4,
                system_timestamp_ns: 300,
                buffer_timestamp_ns: 200,
                frame_id: 3,
            },
            &[1, 2, 3, 4],
        );
        write_test_recorded_frame(
            &right_dir,
            &Metadata {
                camera_id: "camera-a".to_string(),
                frame_index: 0,
                width: 4,
                height: 1,
                payload_bytes: 4,
                system_timestamp_ns: 320,
                buffer_timestamp_ns: 210,
                frame_id: 4,
            },
            &[5, 6, 7, 8],
        );

        let capacity = 4;
        let (left_tx, left_rx) = bounded::<PipelineFrame>(capacity);
        let (right_tx, right_rx) = bounded::<PipelineFrame>(capacity);
        let (left_handles, left_output_rx) = spawn_test_pipeline_graph(left_rx, capacity);
        let (right_handles, right_output_rx) = spawn_test_pipeline_graph(right_rx, capacity);
        let replay_dir = temp_dir.clone();

        let replay_handle = thread::spawn(move || {
            replay_recorded_session(replay_dir, left_tx, right_tx);
        });

        let left_output = left_output_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("left pipeline should produce one output frame");
        let right_output = right_output_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("right pipeline should produce one output frame");

        replay_handle
            .join()
            .expect("recorded-session replay thread should complete");

        assert_eq!(left_output.data(), &[1, 2, 3, 4]);
        assert_eq!(left_output.context().timestamp(), 305);
        assert_eq!(right_output.data(), &[5, 6, 7, 8]);
        assert_eq!(right_output.context().timestamp(), 325);

        for handle in left_handles.into_iter().chain(right_handles) {
            handle
                .join()
                .expect("pipeline stage thread should complete");
        }

        assert!(left_output_rx.try_iter().next().is_none());
        assert!(right_output_rx.try_iter().next().is_none());

        let _ = fs::remove_dir_all(temp_dir);
    }

    
    #[test]
    #[ignore = "manual smoke test for pushing one local file's bytes through the full pipeline graph"]
    fn manual_local_file_crosses_full_pipeline_stage_graph() {
        let file_path = env::var("MJOLNIR_PIPELINE_TEST_FILE")
            .expect("set MJOLNIR_PIPELINE_TEST_FILE to a local file path");
        let file_bytes = fs::read(&file_path)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", file_path));

        let capacity = 2;
        let (tx_in, rx_stage1) = bounded::<PipelineFrame>(capacity);
        let (handles, rx_output) = spawn_test_pipeline_graph(rx_stage1, capacity);

        let input_timestamp = 50u64;
        tx_in.send(PipelineFrame::new(
            file_bytes.clone(),
            Context::new(input_timestamp),
        ))
        .expect("send manual file frame into pipeline");
        drop(tx_in);

        let output_frame = rx_output
            .recv_timeout(Duration::from_secs(5))
            .expect("pipeline should produce one output frame");

        assert_eq!(output_frame.data(), file_bytes.as_slice());
        assert_eq!(output_frame.context().timestamp(), input_timestamp + 5);

        for handle in handles {
            handle
                .join()
                .expect("pipeline stage thread should complete");
        }
    }

    fn spawn_test_pipeline_graph(
        rx_stage1: crossbeam::channel::Receiver<PipelineFrame>,
        capacity: usize,
    ) -> (Vec<thread::JoinHandle<()>>, crossbeam::channel::Receiver<PipelineFrame>) {
        let (tx_stage1, rx_stage2) = bounded::<PipelineFrame>(capacity);
        let (tx_stage2, rx_stage3) = bounded::<PipelineFrame>(capacity);
        let (tx_stage3, rx_stage4) = bounded::<PipelineFrame>(capacity);
        let (tx_stage4, rx_stage5) = bounded::<PipelineFrame>(capacity);
        let (tx_stage5, rx_output) = bounded::<PipelineFrame>(capacity);

        let handles = vec![
            PipelineStage::new(rx_stage1, tx_stage1, undistortion).spawn(),
            PipelineStage::new(rx_stage2, tx_stage2, intensity_normalization).spawn(),
            PipelineStage::new(rx_stage3, tx_stage3, forward_downsampled_copy).spawn(),
            PipelineStage::new(rx_stage4, tx_stage4, mog2).spawn(),
            PipelineStage::new(rx_stage5, tx_stage5, contour).spawn(),
        ];

        (handles, rx_output)
    }

    fn write_test_recorded_frame(dir: &PathBuf, metadata: &Metadata, bytes: &[u8]) {
        let frame_name = format!("frame_{:04}", metadata.frame_index);
        fs::write(dir.join(format!("{frame_name}.raw")), bytes).expect("write frame raw");
        fs::write(
            dir.join(format!("{frame_name}.json")),
            serde_json::to_vec_pretty(metadata).expect("serialize metadata"),
        )
        .expect("write frame metadata");
    }
}
