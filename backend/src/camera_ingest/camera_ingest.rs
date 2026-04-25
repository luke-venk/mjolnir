use crate::pipeline::Frame;

// Ingests frames from the cameras using the GigEVision API, and enqueues
// the frames into the camera ingest sender for the camera's pipeline to
// begin processing.
pub fn ingest_frames(_tx: crossbeam::channel::Sender<Frame>) {
    // TODO(#3): Implement Camera Ingest.
    todo!();
}
