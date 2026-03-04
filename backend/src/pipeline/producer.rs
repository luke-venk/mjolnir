use crate::hardware::{CameraId, Frame};
use super::queue::Queue;
use std::{sync::{
    Arc,
    atomic::{AtomicBool, Ordering}
}, thread, time::Duration};

pub fn producer_run(_camera_id: CameraId, queue: Queue, running: Arc<AtomicBool>) {
    // TODO: implement camera capture code here. Remove dummy stuff.
    // Dummy code is to just add an increasing count every second.
    let mut dummy_counter: u64 = 0;
    while running.load(Ordering::Relaxed) {
        let dummy_frame = Frame::new(dummy_counter);
        let _ = queue.enqueue(dummy_frame);
        dummy_counter += 1;
        thread::sleep(Duration::from_millis(1000));
    }
}
