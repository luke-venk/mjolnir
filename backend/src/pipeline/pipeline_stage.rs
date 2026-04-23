use crate::pipeline::Frame;
use crossbeam::channel::{Receiver, Sender};
use std::thread;

pub struct PipelineStage<F>
where
    F: Fn(Frame) -> Frame + Send + 'static,
{
    rx: Receiver<Frame>,
    tx: Sender<Frame>,
    function: F,
}

impl<F> PipelineStage<F>
where
    F: Fn(Frame) -> Frame + Send + 'static,
{
    pub fn new(rx: Receiver<Frame>, tx: Sender<Frame>, function: F) -> Self {
        Self { rx, tx, function }
    }

    pub fn spawn(self) -> thread::JoinHandle<()> {
        thread::spawn(move || {
            for frame_in in self.rx.iter() {
                let frame_out = (self.function)(frame_in);
                self.tx
                    .send(frame_out)
                    .expect("Error sending processed frame to next stage.");
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::test_utils::generate_frame;

    #[test]
    fn test_can_send_frame_through_pipeline_stage() {
        let frame_in = generate_frame(69, 1738, crate::camera::Resolution::FullHD);

        let (tx_in, rx_pipe) = crossbeam::channel::bounded::<Frame>(3);
        let (tx_pipe, rx_out) = crossbeam::channel::bounded::<Frame>(3);

        let my_function = |f: Frame| -> Frame {
            generate_frame(67, f.context().timestamp() + 1, f.context().resolution())
        };

        let pipeline_stage = PipelineStage::new(rx_pipe, tx_pipe, my_function);
        let _ = pipeline_stage.spawn();
        tx_in.send(frame_in).unwrap();
        let frame_out = rx_out.recv().unwrap();

        assert_eq!(frame_out.context().timestamp(), 1739);
        // Check data stayed the same.
        for &pixel in frame_out.data_as_arr().iter() {
            assert_eq!(pixel, 67);
        }
    }
}
