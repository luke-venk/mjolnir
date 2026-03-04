/**
 * Implementation of the producer-consumer queue used by our pipeline.
 * There will be one queue for each pipeline (i.e. one queue for each
 * of the 2 cameras).
 *
 * To use thread safe queues, we will utilize the crossbeam-channel crate
 * to provide high-performance, producer-consumer queues for concurrent
 * message passing. The documentation for crossbeam-channel can be found here:
 * https://docs.rs/crossbeam-channel/latest/crossbeam_channel/
 *
 * We will use a bounded channel to maintain a rolling buffer storing up
 * to 10 frames. We will also use a drop-oldest policy so that, if the queue
 * fills up while analyzing the frames, we will drop the oldest frame, as
 * to avoid working on analyzing stale data.
 *
 */
use crate::hardware::Frame;
use crossbeam::channel::{Receiver, Sender, RecvError, TryRecvError, TrySendError, bounded};

#[derive(Debug, Clone)]
pub struct Queue {
    // Transmitter that sends messages across channel (producer-facing).
    tx: Sender<Frame>,
    // Reciever that recieves messages from channel (consumer-facing).
    rx: Receiver<Frame>,
}

impl Queue {
    /**
     * Constructor that takes in the size of the rolling buffer and
     * creates a bounded, thread-safe crossbeam-channel queue.
     */
    pub fn new(capacity: usize) -> Self {
        let (tx, rx) = bounded(capacity);
        Self { tx, rx }
    }

    /**
     * Producer facing method that pushes a new frame to the queue.
     * Enforces that the size of the queue is not larger than the capacity.
     *
     * The default behavior of a bounded channel is to simply block sending
     * whenever the queue is at capacity. This is not the behavior we want,
     * since we'd rather drop the oldest frame rather than block the newest
     * frames. Thus, to get around this, if the queue is full, we'll purposely
     * drop the oldest frame before reattempting to send the frame.
     */
    pub fn enqueue(&self, mut frame: Frame) -> Result<(), String> {
        // Uses a loop so it can continuously drop the oldest frames until
        // there is room to enqueue the newest one.
        loop {
            match self.tx.try_send(frame) {
                Ok(()) => return Ok(()),
                Err(TrySendError::Full(f)) => {
                    // Since send failed, the error returns the same frame. So
                    // reassign frame to be this captured f.
                    frame = f;
                    // Drop the oldest frame in favor of enqueueing the new frame.
                    match self.rx.try_recv() {
                        Ok(_) => {},
                        Err(TryRecvError::Empty) => {},
                        Err(TryRecvError::Disconnected) => {
                            return Err("Error: The receiver channel has been disconnected.".into())
                        }
                    }
                }
                Err(TrySendError::Disconnected(_)) => {
                    return Err("Error: The sender channel has been disconnected.".into())
                }
            }
        }
    }

    /**
     * Consumer facing method that blocks until a frame is available, then
     * pops the oldest frame from the queue so the consumer can use computer
     * vision to analyze it.
     */
    pub fn dequeue(&self) -> Result<Option<Frame>, String> {
        match self.rx.recv() {
            Ok(frame) => Ok(Some(frame)),
            Err(RecvError) => {
                Err("Error: The receiver channel has been disconnected.".into())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hardware::Frame;

    #[test]
    fn test_enqueue_then_dequeue_one() {
        let q = Queue::new(3);
        assert!(q.enqueue(Frame::new(vec![1, 2, 3, 4], 1)).is_ok());
        
        let frame = q.dequeue().unwrap().expect("Dequeuing failed and should have succeeded.");

        assert_eq!(frame.timestamp(), 1);
    }

    #[test]
    fn test_drop_oldest_when_full() {
        let q = Queue::new(3);
        assert!(q.enqueue(Frame::new(vec![1, 2, 3, 4], 1)).is_ok());
        assert!(q.enqueue(Frame::new(vec![1, 2, 3, 4], 2)).is_ok());
        assert!(q.enqueue(Frame::new(vec![1, 2, 3, 4], 3)).is_ok());
        assert!(q.enqueue(Frame::new(vec![1, 2, 3, 4], 4)).is_ok());
        // Note: The first frame should have been dropped since it's the most stale.

        let frame1 = q.dequeue().unwrap().expect("Dequeuing failed and should have succeeded.");
        let frame2 = q.dequeue().unwrap().expect("Dequeuing failed and should have succeeded.");
        let frame3 = q.dequeue().unwrap().expect("Dequeuing failed and should have succeeded.");

        assert_eq!(frame1.timestamp(), 2);
        assert_eq!(frame2.timestamp(), 3);
        assert_eq!(frame3.timestamp(), 4);
    }

}