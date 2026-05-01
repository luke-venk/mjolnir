use super::PipelineStage;
use crate::camera_ingest::ingest_frames;
use crate::computer_vision::{contour, forward_downsampled_copy, mog2, undistortion};
use crate::pipeline::{CameraId, Frame};
use crossbeam::channel::{Receiver, Sender, bounded};
use std::thread::{self, JoinHandle};

pub struct Pipeline {
    handles: Vec<JoinHandle<()>>,
}

#[allow(dead_code)]
impl Pipeline {
    // The live-camera constructor creates the ingest thread, all inter-stage
    // channels, stage workers, and output forwarding thread.
    pub fn new(
        camera_id: CameraId,
        camera_name: String,
        capacity_per_channel: usize,
        frame_output_tx: Sender<Frame>,
    ) -> Self {
        let (tx_ingest, rx_stage1) = bounded::<Frame>(capacity_per_channel);
        let handle_ingest = thread::spawn(move || {
            ingest_frames(camera_id, camera_name, tx_ingest);
        });

        let mut pipeline = Self::from_receiver(rx_stage1, capacity_per_channel, frame_output_tx);
        pipeline.handles.insert(0, handle_ingest);
        pipeline
    }

    // Builds one pipeline stage graph around an incoming frame receiver.
    pub fn from_receiver(
        rx_stage1: Receiver<Frame>,
        capacity_per_channel: usize,
        frame_output_tx: Sender<Frame>,
    ) -> Self {
        let (tx_stage1, rx_stage2) = bounded::<Frame>(capacity_per_channel);
        let (tx_stage2, rx_stage3) = bounded::<Frame>(capacity_per_channel);
        let (tx_stage3, rx_stage4) = bounded::<Frame>(capacity_per_channel);
        let (tx_stage4, rx_output) = bounded::<Frame>(capacity_per_channel);

        let handle_stage1 = PipelineStage::new(rx_stage1, tx_stage1, undistortion).spawn();
        let handle_stage2 =
            PipelineStage::new(rx_stage2, tx_stage2, forward_downsampled_copy).spawn();
        let handle_stage3 = PipelineStage::new(rx_stage3, tx_stage3, mog2).spawn();
        let handle_stage4 = PipelineStage::new(rx_stage4, tx_stage4, contour).spawn();

        let handle_output = thread::spawn(move || {
            for frame in rx_output.iter() {
                frame_output_tx
                    .send(frame)
                    .expect("Error sending processed frame to aggregator.");
            }
        });

        Self {
            handles: vec![
                handle_stage1,
                handle_stage2,
                handle_stage3,
                handle_stage4,
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

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::Duration;
    use std::time::{SystemTime, UNIX_EPOCH};

    use crossbeam::channel::bounded;

    use super::Pipeline;
    use crate::camera::record::writer::Metadata;
    use crate::camera_ingest::replay_recorded_session;
    use crate::computer_vision::{contour, forward_downsampled_copy, mog2, undistortion};
    use crate::pipeline::{CameraId, Context, Frame as PipelineFrame, PipelineStage};

    const LEFT_CAM_DIR: &str = "left_cam";
    const RIGHT_CAM_DIR: &str = "right_cam";

    #[test]
    fn pipeline_from_receiver_starts_and_stops_after_input_channel_closes() {
        let (tx, rx) = bounded::<PipelineFrame>(2);
        let (output_tx, output_rx) = bounded::<PipelineFrame>(2);
        let pipeline = Pipeline::from_receiver(rx, 2, output_tx);

        tx.send(PipelineFrame::new(
            vec![1, 2, 3, 4].into_boxed_slice(),
            (2, 2),
            Context::new(CameraId::FieldLeft, 1),
        ))
        .expect("send pipeline frame");
        drop(tx);
        let _ = output_rx.recv_timeout(Duration::from_secs(5));

        pipeline.stop();
    }

    #[test]
    fn recorded_session_replay_forwards_frames_into_left_and_right_pipeline_inputs() {
        let temp_dir = env::temp_dir().join(format!(
            "mjolnir_pipeline_replay_test_{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time should be after epoch")
                .as_nanos()
        ));
        let left_dir = temp_dir.join(LEFT_CAM_DIR);
        let right_dir = temp_dir.join(RIGHT_CAM_DIR);
        fs::create_dir_all(&left_dir).expect("create left test dir");
        fs::create_dir_all(&right_dir).expect("create right test dir");

        write_test_recorded_frame(
            &left_dir,
            &Metadata {
                camera_id: "left-cam-serial".to_string(),
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
                camera_id: "right-cam-serial".to_string(),
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
        replay_recorded_session(temp_dir.clone(), left_tx, right_tx);

        let left_frames: Vec<_> = left_rx.try_iter().collect();
        let right_frames: Vec<_> = right_rx.try_iter().collect();

        assert_eq!(left_frames.len(), 1);
        assert_eq!(left_frames[0].raw_bytes_full_resolution().as_ref(), &[1, 2, 3, 4]);
        assert_eq!(left_frames[0].context().camera_id(), CameraId::FieldLeft);
        assert_eq!(left_frames[0].context().camera_buffer_timestamp(), 200);
        assert_eq!(right_frames.len(), 1);
        assert_eq!(right_frames[0].raw_bytes_full_resolution().as_ref(), &[5, 6, 7, 8]);
        assert_eq!(right_frames[0].context().camera_id(), CameraId::FieldRight);
        assert_eq!(right_frames[0].context().camera_buffer_timestamp(), 210);

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn recorded_session_replay_crosses_all_pipeline_stage_slots() {
        let temp_dir = env::temp_dir().join(format!(
            "mjolnir_pipeline_stage_travel_test_{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time should be after epoch")
                .as_nanos()
        ));
        let left_dir = temp_dir.join(LEFT_CAM_DIR);
        let right_dir = temp_dir.join(RIGHT_CAM_DIR);
        fs::create_dir_all(&left_dir).expect("create left test dir");
        fs::create_dir_all(&right_dir).expect("create right test dir");

        write_test_recorded_frame(
            &left_dir,
            &Metadata {
                camera_id: "left-cam-serial".to_string(),
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
                camera_id: "right-cam-serial".to_string(),
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
        let travel_log = Arc::new(Mutex::new(Vec::<(CameraId, usize, u64)>::new()));
        let (left_handles, left_output_rx) =
            spawn_tracking_pipeline_graph(left_rx, capacity, Arc::clone(&travel_log));
        let (right_handles, right_output_rx) =
            spawn_tracking_pipeline_graph(right_rx, capacity, Arc::clone(&travel_log));

        replay_recorded_session(temp_dir.clone(), left_tx, right_tx);

        let left_output = left_output_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("left tracked pipeline should emit one frame");
        let right_output = right_output_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("right tracked pipeline should emit one frame");

        assert_eq!(left_output.context().camera_id(), CameraId::FieldLeft);
        assert_eq!(left_output.context().camera_buffer_timestamp(), 200);
        assert_eq!(right_output.context().camera_id(), CameraId::FieldRight);
        assert_eq!(right_output.context().camera_buffer_timestamp(), 210);

        for handle in left_handles.into_iter().chain(right_handles) {
            handle
                .join()
                .expect("tracked pipeline stage thread should complete");
        }

        let entries = travel_log.lock().expect("travel log lock").clone();
        let left_entries = entries
            .iter()
            .filter(|(camera_id, _, timestamp)| {
                *camera_id == CameraId::FieldLeft && *timestamp == 200
            })
            .count();
        let right_entries = entries
            .iter()
            .filter(|(camera_id, _, timestamp)| {
                *camera_id == CameraId::FieldRight && *timestamp == 210
            })
            .count();

        assert_eq!(
            left_entries, 4,
            "left frame should traverse all 4 stage slots"
        );
        assert_eq!(
            right_entries, 4,
            "right frame should traverse all 4 stage slots"
        );
        for stage_index in 1..=4 {
            assert!(
                entries.iter().any(|(camera_id, idx, timestamp)| {
                    *camera_id == CameraId::FieldLeft && *idx == stage_index && *timestamp == 200
                }),
                "left frame should reach stage slot {stage_index}"
            );
            assert!(
                entries.iter().any(|(camera_id, idx, timestamp)| {
                    *camera_id == CameraId::FieldRight && *idx == stage_index && *timestamp == 210
                }),
                "right frame should reach stage slot {stage_index}"
            );
        }

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
        tx_in
            .send(PipelineFrame::new(
                file_bytes.clone().into_boxed_slice(),
                (file_bytes.len() as u32, 1),
                Context::new(CameraId::FieldLeft, input_timestamp),
            ))
            .expect("send manual file frame into pipeline");
        drop(tx_in);

        let output_frame = rx_output
            .recv_timeout(Duration::from_secs(5))
            .expect("pipeline should produce one output frame");

        assert_eq!(output_frame.raw_bytes_full_resolution().as_ref(), file_bytes.as_slice());
        assert_eq!(output_frame.context().camera_buffer_timestamp(), input_timestamp);

        for handle in handles {
            handle
                .join()
                .expect("pipeline stage thread should complete");
        }
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

    fn spawn_test_pipeline_graph(
        rx_stage1: crossbeam::channel::Receiver<PipelineFrame>,
        capacity: usize,
    ) -> (
        Vec<thread::JoinHandle<()>>,
        crossbeam::channel::Receiver<PipelineFrame>,
    ) {
        let (tx_stage1, rx_stage2) = bounded::<PipelineFrame>(capacity);
        let (tx_stage2, rx_stage3) = bounded::<PipelineFrame>(capacity);
        let (tx_stage3, rx_stage4) = bounded::<PipelineFrame>(capacity);
        let (tx_stage4, rx_output) = bounded::<PipelineFrame>(capacity);

        let handles = vec![
            PipelineStage::new(rx_stage1, tx_stage1, undistortion).spawn(),
            PipelineStage::new(rx_stage2, tx_stage2, forward_downsampled_copy).spawn(),
            PipelineStage::new(rx_stage3, tx_stage3, mog2).spawn(),
            PipelineStage::new(rx_stage4, tx_stage4, contour).spawn(),
        ];

        (handles, rx_output)
    }

    fn spawn_tracking_pipeline_graph(
        rx_stage1: crossbeam::channel::Receiver<PipelineFrame>,
        capacity: usize,
        travel_log: Arc<Mutex<Vec<(CameraId, usize, u64)>>>,
    ) -> (
        Vec<thread::JoinHandle<()>>,
        crossbeam::channel::Receiver<PipelineFrame>,
    ) {
        let (tx_stage1, rx_stage2) = bounded::<PipelineFrame>(capacity);
        let (tx_stage2, rx_stage3) = bounded::<PipelineFrame>(capacity);
        let (tx_stage3, rx_stage4) = bounded::<PipelineFrame>(capacity);
        let (tx_stage4, rx_output) = bounded::<PipelineFrame>(capacity);

        let make_stage = |stage_index: usize,
                          rx: crossbeam::channel::Receiver<PipelineFrame>,
                          tx: crossbeam::channel::Sender<PipelineFrame>| {
            let stage_log = Arc::clone(&travel_log);
            PipelineStage::new(rx, tx, move |frame: PipelineFrame| {
                stage_log
                    .lock()
                    .expect("travel log lock")
                    .push((
                        frame.context().camera_id(),
                        stage_index,
                        frame.context().camera_buffer_timestamp(),
                    ));
                frame
            })
            .spawn()
        };

        let handles = vec![
            make_stage(1, rx_stage1, tx_stage1),
            make_stage(2, rx_stage2, tx_stage2),
            make_stage(3, rx_stage3, tx_stage3),
            make_stage(4, rx_stage4, tx_stage4),
        ];

        (handles, rx_output)
    }
}
