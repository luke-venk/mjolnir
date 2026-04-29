use super::PipelineStage;
use crate::camera_ingest::ingest_frames;
use crate::computer_vision::{ContourTracker, forward_downsampled_copy, mog2, undistortion};
use crate::pipeline::{CameraId, Frame};
use crossbeam::channel::{Sender, bounded};
use std::thread::{self, JoinHandle};

pub struct Pipeline {
    handles: Vec<JoinHandle<()>>,
}

#[allow(dead_code)]
impl Pipeline {
    pub fn new(
        camera_id: CameraId,
        camera_name: String,
        capacity_per_channel: usize,
        frame_output_tx: Sender<Frame>,
    ) -> Self {
        let (tx_ingest, rx_stage1) = bounded::<Frame>(capacity_per_channel);
        let (tx_stage1, rx_stage2) = bounded::<Frame>(capacity_per_channel);
        let (tx_stage2, rx_stage3) = bounded::<Frame>(capacity_per_channel);
        let (tx_stage3, rx_stage4) = bounded::<Frame>(capacity_per_channel);
        let (tx_stage4, rx_output) = bounded::<Frame>(capacity_per_channel);

        let handle_ingest = thread::spawn(move || {
            ingest_frames(camera_id, camera_name, tx_ingest);
        });

        let handle_stage1 = PipelineStage::new(rx_stage1, tx_stage1, undistortion).spawn();
        let handle_stage2 =
            PipelineStage::new(rx_stage2, tx_stage2, forward_downsampled_copy).spawn();
        let handle_stage3 = PipelineStage::new(rx_stage3, tx_stage3, mog2).spawn();

        let mut contour_tracker = ContourTracker::new();
        let handle_stage4 = PipelineStage::new(rx_stage4, tx_stage4, move |frame| {
            contour_tracker.process_frame(frame)
        })
        .spawn();

        let handle_output = thread::spawn(move || {
            for frame in rx_output.iter() {
                frame_output_tx
                    .send(frame)
                    .expect("Error sending processed frame to aggregator.");
            }
        });

        Self {
            handles: vec![
                handle_ingest,
                handle_stage1,
                handle_stage2,
                handle_stage3,
                handle_stage4,
                handle_output,
            ],
        }
    }

    pub fn stop(self) {
        for handle in self.handles {
            let _ = handle.join();
        }
    }
}
