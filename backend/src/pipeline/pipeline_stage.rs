use std::thread;

use crate::schemas::Frame;
use crossbeam::channel::{Receiver, Sender};

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
    use crate::schemas::Context;
    
    #[test]
    fn test_can_send_frame_through_pipeline_stage() {
        let frame_in = Frame::new(vec![6, 9, 6, 9], Context::new(1738));

        let (tx_in, rx_pipe) = crossbeam::channel::bounded::<Frame>(3);
        let (tx_pipe, rx_out) = crossbeam::channel::bounded::<Frame>(3);
        
        let my_function = |f: Frame| -> Frame {
            let new_data = vec![6, 7, 6, 7];
            let new_metadata = f.context().metadata() + 1;
            Frame::new(new_data, Context::new(new_metadata))
        };

        let pipeline_stage = PipelineStage::new(rx_pipe, tx_pipe, my_function);
        let _ = pipeline_stage.spawn();
        tx_in.send(frame_in).unwrap();
        let frame_out = rx_out.recv().unwrap();

        assert_eq!(frame_out.data(), vec![6, 7, 6, 7]);
        assert_eq!(frame_out.context().metadata(), 1739);
    }
}