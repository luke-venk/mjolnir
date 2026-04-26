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
    use std::time::Duration;

    use crossbeam::channel::bounded;

    use super::Pipeline;
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
    #[ignore = "manual smoke test for pushing a local frame file through the full pipeline stage graph"]
    fn manual_file_frame_crosses_full_pipeline_stage_graph() {
        let frame_path = env::var("MJOLNIR_PIPELINE_TEST_FRAME")
            .expect("set MJOLNIR_PIPELINE_TEST_FRAME to a local frame file path");
        let frame_bytes = fs::read(&frame_path)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", frame_path));

        let (tx_in, rx_stage1) = bounded::<PipelineFrame>(2);
        let (tx_stage1, rx_stage2) = bounded::<PipelineFrame>(2);
        let (tx_stage2, rx_stage3) = bounded::<PipelineFrame>(2);
        let (tx_stage3, rx_stage4) = bounded::<PipelineFrame>(2);
        let (tx_stage4, rx_stage5) = bounded::<PipelineFrame>(2);
        let (tx_stage5, rx_output) = bounded::<PipelineFrame>(2);

        let handles = vec![
            PipelineStage::new(rx_stage1, tx_stage1, undistortion).spawn(),
            PipelineStage::new(rx_stage2, tx_stage2, intensity_normalization).spawn(),
            PipelineStage::new(rx_stage3, tx_stage3, forward_downsampled_copy).spawn(),
            PipelineStage::new(rx_stage4, tx_stage4, mog2).spawn(),
            PipelineStage::new(rx_stage5, tx_stage5, contour).spawn(),
        ];

        let input_timestamp = 50u64;
        tx_in.send(PipelineFrame::new(
            frame_bytes.clone(),
            Context::new(input_timestamp),
        ))
        .expect("send manual file frame into pipeline");
        drop(tx_in);

        let output_frame = rx_output
            .recv_timeout(Duration::from_secs(5))
            .expect("pipeline should produce one output frame");

        println!(
            "pipeline test: loaded {} ({} bytes)",
            frame_path,
            frame_bytes.len()
        );
        println!(
            "pipeline test: input timestamp {}, output timestamp {}",
            input_timestamp,
            output_frame.context().timestamp()
        );

        assert_eq!(output_frame.data(), frame_bytes.as_slice());
        assert_eq!(output_frame.context().timestamp(), input_timestamp + 5);

        for handle in handles {
            handle
                .join()
                .expect("pipeline stage thread should complete");
        }
    }
}
