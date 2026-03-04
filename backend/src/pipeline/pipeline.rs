/**
 * This file will handle the implementation of our computer vision pipeline.
 * Each camera will have its own pipeline consisting of the following:
 *   (1) Producer
 *   (2) Queue
 *   (3) Consumer
 * 
 * The producer will handle ingesting the frames from the cameras and enqueue
 * into the queue. The consumer will be a worker that dequeues from the queue
 * and perform the computer vision on the implement, returning a message 
 * communicating the distance the object landed and whether or not the object
 * landed out of bounds. Each producer and consumer will be on its own thread.
 * 
 * The queue will be a thread-safe queue that allows both producers and
 * consumer to enqueue/dequeue while avoiding data races.
 * 
 * This architecture is essential to allow parallelism and to decouple 
 * ingesting camera footage from the heavy computer vision. For example, if 
 * computer vision is taking a very long time to analyze one frame, we don't
 * want that to block our pipeline from receiving the next frame. Furthermore,
 * we can run producers and consumers on their own threads to speed things up.
 */

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread::{self, JoinHandle};
use crate::hardware::CameraId;
use super::queue::Queue;
use super::producer::producer_run;
use super::consumer::consumer_run;

pub struct Pipeline {
    _camera_id: CameraId,
    queue: Queue,
    running: Arc<AtomicBool>,
    producer_handle: Option<JoinHandle<()>>,
    consumer_handle: Option<JoinHandle<()>>,
}

impl Pipeline {
    pub fn new(_camera_id: CameraId, capacity: usize) -> Self {
        Self {
            _camera_id,
            queue: Queue::new(capacity),
            running: Arc::new(AtomicBool::new(true)),
            producer_handle: None,
            consumer_handle: None,
        }
    }

    pub fn start(&mut self) {
        // The "_p" prefix corresponds to producer, and "_c" corresponds to consumer.
        let running_p = Arc::clone(&self.running);
        let running_c = Arc::clone(&self.running);

        // Clone the handle for the queue for both the producer and consumer to access.
        let queue_p = self.queue.clone();
        let queue_c = self.queue.clone();

        let camera_id = self._camera_id;
        // Spawn producer on its own thread.
        self.producer_handle = Some(thread::spawn(move || {
            producer_run(camera_id, queue_p, running_p);
        }));

        // Spawn consumer on its own thread.
        self.consumer_handle = Some(thread::spawn(move || {
            consumer_run(queue_c, running_c);
        }));
    }

    // Safely brings down both producer and consumer thread when system stops,
    // and blocks the main thread until these come down.
    pub fn stop(&mut self) {
        // Set `running` AtomicBool to false.
        self.running.store(false, Ordering::Relaxed);

        // Block until producer thread exits and take away handle.
        if let Some(h) = self.producer_handle.take() {
            let _ = h.join();
        }

        // Block until consumer thread exits and take away handle.
        if let Some(h) = self.consumer_handle.take() {
            let _ = h.join();
        }
    }

}
