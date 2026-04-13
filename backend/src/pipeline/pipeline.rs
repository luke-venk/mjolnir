use super::PipelineStage;
use crate::computer_vision::{
    contour, forward_downsampled_copy, intensity_normalization, mog2, undistortion,
};
use crate::schemas::{CameraId, Frame};
use crossbeam::channel::{Receiver, bounded};
use std::thread::{self, JoinHandle};

pub struct Pipeline {
    _camera_id: CameraId,
    handles: Vec<JoinHandle<()>>,
}

#[allow(dead_code)]
impl Pipeline {
    // Builds one pipeline stage graph around an incoming frame receiver.
    pub fn from_receiver(
        camera_id: CameraId,
        rx_stage1: Receiver<Frame>,
        capacity_per_channel: usize,
    ) -> Self {
        let (tx_stage1, rx_stage2) = bounded::<Frame>(capacity_per_channel);
        let (tx_stage2, rx_stage3) = bounded::<Frame>(capacity_per_channel);
        let (tx_stage3, rx_stage4) = bounded::<Frame>(capacity_per_channel);
        let (tx_stage4, rx_stage5) = bounded::<Frame>(capacity_per_channel);
        let (tx_stage5, rx_output) = bounded::<Frame>(capacity_per_channel);

        let handle_stage1 = PipelineStage::new(rx_stage1, tx_stage1, undistortion).spawn();
        let handle_stage2 =
            PipelineStage::new(rx_stage2, tx_stage2, intensity_normalization).spawn();
        let handle_stage3 =
            PipelineStage::new(rx_stage3, tx_stage3, forward_downsampled_copy).spawn();
        let handle_stage4 = PipelineStage::new(rx_stage4, tx_stage4, mog2).spawn();
        let handle_stage5 = PipelineStage::new(rx_stage5, tx_stage5, contour).spawn();

        let handle_output = thread::spawn(move || {
            for _frame in rx_output.iter() {
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
