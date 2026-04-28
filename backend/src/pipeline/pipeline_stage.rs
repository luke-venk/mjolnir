use crossbeam::channel::{Receiver, Sender};
use std::thread;

pub struct PipelineStage<In, Out, F>
where
    In: Send + 'static,
    Out: Send + 'static,
    F: FnMut(In) -> Out + Send + 'static,
{
    rx: Receiver<In>,
    tx: Sender<Out>,
    function: F,
}

impl<In, Out, F> PipelineStage<In, Out, F>
where
    In: Send + 'static,
    Out: Send + 'static,
    F: FnMut(In) -> Out + Send + 'static,
{
    pub fn new(rx: Receiver<In>, tx: Sender<Out>, function: F) -> Self {
        Self { rx, tx, function }
    }

    pub fn spawn(mut self) -> thread::JoinHandle<()> {
        thread::spawn(move || {
            for input in self.rx.iter() {
                let output = (self.function)(input);
                self.tx
                    .send(output)
                    .expect("Error sending processed pipeline output to next stage.");
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::{CameraId, Context, Frame};

    #[test]
    fn test_can_send_frame_through_homogeneous_pipeline_stage() {
        let frame_in = Frame::new(
            vec![6, 9, 6, 9].into_boxed_slice(),
            (2, 2),
            Context::new(CameraId::FieldLeft, 1738),
        );

        let (tx_in, rx_pipe) = crossbeam::channel::bounded::<Frame>(3);
        let (tx_pipe, rx_out) = crossbeam::channel::bounded::<Frame>(3);

        let my_function = |f: Frame| -> Frame {
            Frame::new(
                vec![6, 7, 6, 7].into_boxed_slice(),
                (2, 2),
                *f.context(),
            )
        };

        let pipeline_stage = PipelineStage::new(rx_pipe, tx_pipe, my_function);
        let _ = pipeline_stage.spawn();
        tx_in.send(frame_in).unwrap();
        let frame_out = rx_out.recv().unwrap();

        assert_eq!(frame_out.raw_bytes_full_resolution().as_ref(), &[6, 7, 6, 7]);
        assert_eq!(frame_out.context().camera_id(), CameraId::FieldLeft);
        assert_eq!(frame_out.context().camera_buffer_timestamp(), 1738);
    }

    #[test]
    fn test_can_send_frame_through_heterogeneous_pipeline_stage() {
        let frame_in = Frame::new(
            vec![6, 9, 6, 9].into_boxed_slice(),
            (2, 2),
            Context::new(CameraId::FieldRight, 88),
        );

        let (tx_in, rx_pipe) = crossbeam::channel::bounded::<Frame>(3);
        let (tx_pipe, rx_out) = crossbeam::channel::bounded::<usize>(3);

        let my_function = |f: Frame| -> usize { f.raw_bytes_full_resolution().len() };

        let pipeline_stage = PipelineStage::new(rx_pipe, tx_pipe, my_function);
        let _ = pipeline_stage.spawn();
        tx_in.send(frame_in).unwrap();
        let output = rx_out.recv().unwrap();

        assert_eq!(output, 4);
    }

    #[test]
    fn test_can_send_frame_through_stateful_pipeline_stage() {
        let frame_one = Frame::new(
            vec![1, 1, 1, 1].into_boxed_slice(),
            (2, 2),
            Context::new(CameraId::FieldLeft, 1),
        );
        let frame_two = Frame::new(
            vec![2, 2, 2, 2].into_boxed_slice(),
            (2, 2),
            Context::new(CameraId::FieldLeft, 2),
        );

        let (tx_in, rx_pipe) = crossbeam::channel::bounded::<Frame>(3);
        let (tx_pipe, rx_out) = crossbeam::channel::bounded::<usize>(3);

        let mut count = 0usize;
        let my_function = move |_f: Frame| -> usize {
            count += 1;
            count
        };

        let pipeline_stage = PipelineStage::new(rx_pipe, tx_pipe, my_function);
        let _ = pipeline_stage.spawn();
        tx_in.send(frame_one).unwrap();
        tx_in.send(frame_two).unwrap();

        assert_eq!(rx_out.recv().unwrap(), 1);
        assert_eq!(rx_out.recv().unwrap(), 2);
    }
}
