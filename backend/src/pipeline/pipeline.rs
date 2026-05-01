use super::PipelineStage;
use crate::camera_ingest::ingest_frames;
use crate::computer_vision::{contour, contour_with_options, forward_downsampled_copy, mog2, mog2_with_options, undistortion};
use crate::pipeline::{CameraId, Frame};
use crossbeam::channel::{Sender, bounded};
use std::thread::{self, JoinHandle};

#[derive(Debug, Clone, Copy, Default)]
pub struct PipelineStageOptions {
    pub rerun_4k_mode: bool,
}

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
        stage_options: PipelineStageOptions,
    ) -> Self {
        let (tx_ingest, rx_stage1) = bounded::<Frame>(capacity_per_channel);
        let (tx_stage1, rx_stage2) = bounded::<Frame>(capacity_per_channel);
        let (tx_stage2, rx_stage3) = bounded::<Frame>(capacity_per_channel);
        let (tx_stage3, rx_stage4) = bounded::<Frame>(capacity_per_channel);
        let (tx_stage4, rx_output) = bounded::<Frame>(capacity_per_channel);

        let handle_ingest = thread::spawn(move || {
            ingest_frames(camera_id, camera_name, tx_ingest);
        });

        let handle_stage1 = PipelineStage::new(rx_stage1, tx_stage1, move |frame| {
            if stage_options.rerun_4k_mode {
                frame
            } else {
                undistortion(frame)
            }
        })
        .spawn();
        let handle_stage2 = PipelineStage::new(rx_stage2, tx_stage2, move |frame| {
            if stage_options.rerun_4k_mode {
                frame
            } else {
                forward_downsampled_copy(frame)
            }
        })
        .spawn();
        let handle_stage3 = PipelineStage::new(rx_stage3, tx_stage3, move |frame| {
            mog2_with_options(frame, stage_options)
        })
        .spawn();
        let handle_stage4 = PipelineStage::new(rx_stage4, tx_stage4, move |frame| {
            contour_with_options(frame, stage_options)
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
