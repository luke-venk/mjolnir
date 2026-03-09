use super::PipelineStage;
use crate::camera_ingest::ingest_frames;
use crate::computer_vision::{
    contour, forward_downsampled_copy, intensity_normalization, mog2, undistortion,
};
use crate::schemas::{CameraId, Frame};
use crossbeam::channel::bounded;
use std::thread::{self, JoinHandle};

pub struct Pipeline {
    _camera_id: CameraId,
    handles: Vec<JoinHandle<()>>,
}

#[allow(dead_code)]
impl Pipeline {
    // The Pipeline constructor will create all inter-stage message channels,
    // create the PipelineStages, spawn each stage, and return a Pipeline.
    // Calling this function automatically starts each of the pipeline
    // stages.
    pub fn new(_camera_id: CameraId, capacity_per_channel: usize) -> Self {
        // TODO(#6): Implement Custom Queue Policy.

        // Define inter-stage message channels for thread-safe message sharing.
        let (tx_ingest, rx_stage1) = bounded::<Frame>(capacity_per_channel);
        let (tx_stage1, rx_stage2) = bounded::<Frame>(capacity_per_channel);
        let (tx_stage2, rx_stage3) = bounded::<Frame>(capacity_per_channel);
        let (tx_stage3, rx_stage4) = bounded::<Frame>(capacity_per_channel);
        let (tx_stage4, rx_stage5) = bounded::<Frame>(capacity_per_channel);
        let (tx_stage5, rx_output) = bounded::<Frame>(capacity_per_channel);

        // Spawn a thread to handle camera ingest from GigEVision API here.
        let handle_ingest = thread::spawn(move || {
            ingest_frames(tx_ingest);
        });

        // Create and start each of the pipeline stages.
        // Note, storing handles instead of directly storing the pipeline stages since spawn() moves
        // the pipeline stage into the thread anyway. Just store handles so can join() later on.
        let handle_stage1 = PipelineStage::new(rx_stage1, tx_stage1, undistortion).spawn();
        let handle_stage2 =
            PipelineStage::new(rx_stage2, tx_stage2, intensity_normalization).spawn();
        let handle_stage3 =
            PipelineStage::new(rx_stage3, tx_stage3, forward_downsampled_copy).spawn();
        let handle_stage4 = PipelineStage::new(rx_stage4, tx_stage4, mog2).spawn();
        let handle_stage5 = PipelineStage::new(rx_stage5, tx_stage5, contour).spawn();

        // Spawn a thread to handle reporting pipeline outputs to server.
        let handle_output = thread::spawn(move || {
            for _frame in rx_output.iter() {
                // TODO: forward results to output.
            }
        });

        Self {
            _camera_id,
            handles: vec![
                handle_ingest,
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
