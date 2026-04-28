use super::PipelineStage;
use crate::camera_ingest::ingest_frames;
use crate::computer_vision::{contour, forward_downsampled_copy, mog2, undistortion};
use crate::pipeline::{CameraId, ContourOutput, Frame};
use crossbeam::channel::{Sender, bounded};
use std::thread::{self, JoinHandle};

pub struct Pipeline {
    handles: Vec<JoinHandle<()>>,
}

#[allow(dead_code)]
impl Pipeline {
    // The Pipeline constructor will create all inter-stage message channels,
    // create the PipelineStages, spawn each stage, and return a Pipeline.
    // Calling this function automatically starts each of the pipeline
    // stages.
    pub fn new(
        camera_id: CameraId,
        camera_name: String,
        capacity_per_channel: usize,
        contour_output_tx: Sender<ContourOutput>,
    ) -> Self {
        // Define inter-stage message channels for thread-safe message sharing.
        let (tx_ingest, rx_stage1) = bounded::<Frame>(capacity_per_channel);
        let (tx_stage1, rx_stage2) = bounded::<Frame>(capacity_per_channel);
        let (tx_stage2, rx_stage3) = bounded::<Frame>(capacity_per_channel);
        let (tx_stage3, rx_stage4) = bounded::<Frame>(capacity_per_channel);
        let (tx_stage4, rx_output) = bounded::<Frame>(capacity_per_channel);

        // Input: Camera Ingest.
        // Spawn a thread to handle camera ingest from GigEVision API here.
        let handle_ingest = thread::spawn(move || {
            ingest_frames(camera_id, camera_name, tx_ingest);
        });

        // Stage 1: Undistortion.
        let handle_stage1 = PipelineStage::new(rx_stage1, tx_stage1, undistortion).spawn();
        // Stage 2: Forward Downsampled Copy.
        let handle_stage2 =
            PipelineStage::new(rx_stage2, tx_stage2, forward_downsampled_copy).spawn();

        // Stage 3: Mog2.
        let handle_stage3 = PipelineStage::new(rx_stage3, tx_stage3, mog2).spawn();

        // Stage 4: Contour.
        let handle_stage4 = PipelineStage::new(rx_stage4, tx_stage4, contour).spawn();

        // Output: Pixel coordinates.
        // Spawn a thread to handle reporting pipeline outputs to math triangulation.
        let handle_output = thread::spawn(move || {
            for frame in rx_output.iter() {
                let contour_output = ContourOutput::from(frame);
                contour_output_tx
                    .send(contour_output)
                    .expect("Error sending contour output to aggregator.");
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

    // Safely brings down all pipeline stage threads when system stops,
    // and blocks the main thread until these come down.
    pub fn stop(self) {
        for handle in self.handles {
            let _ = handle.join();
        }
    }
}
