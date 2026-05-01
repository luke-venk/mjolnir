use crate::pipeline::{CameraId, Context, Frame};

// Ingests frames from the cameras using the GigEVision API, and enqueues
// the frames into the camera ingest sender for the camera's pipeline to
// begin processing.
pub fn ingest_frames(
    camera_id: CameraId,
    camera_name: String,
    tx: crossbeam::channel::Sender<Frame>,
) {
    // TODO(#3): Implement Camera Ingest using the configured camera name/identity.
    let _camera_name = camera_name;

    let data = vec![1, 2, 3, 4].into_boxed_slice();
    let context = Context::new(camera_id, 1);
    let _ = tx.send(Frame::new(data, (2, 2), context));
}
