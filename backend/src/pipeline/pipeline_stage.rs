use std::thread;

use crate::hardware::Frame;
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
