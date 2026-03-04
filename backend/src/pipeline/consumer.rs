use super::queue::Queue;
use std::{sync::{
    Arc,
    atomic::{AtomicBool, Ordering}
}, thread, time::Duration};

pub fn consumer_run(queue: Queue, running: Arc<AtomicBool>) {
    // TODO: implement computer vision here. Remove dummy stuff.
    // Dummy code is to just pull off the queue and print the frame's timestamp.
    // Simulates the consumer to be slower than the producer.
    while running.load(Ordering::Relaxed) {
        if let Ok(Some(frame)) = queue.dequeue() {
            thread::sleep(Duration::from_millis(1500));
            println!("Processed frame {}", frame.timestamp());
        }
    }
}