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

pub struct Pipeline;
