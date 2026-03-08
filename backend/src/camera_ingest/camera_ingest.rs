use std::{thread, time::Duration};

use crate::hardware::{Frame, Context};

// Ingests frames from the cameras using the GigEVision API, and enqueues
// the frames into the camera ingest sender for the camera's pipeline to
// begin processing.
pub fn ingest_frames(tx: crossbeam::channel::Sender<Frame>){
    // TODO(#3): Implement Camera Ingest.
    loop {
        let data = vec![1, 2, 3, 4];
        let context = Context::new(1);
        println!("Ingesting frame with metadata: {:?}", context);  // TODO: remove
        let _ = tx.send(Frame::new(data, context));
        thread::sleep(Duration::from_millis(3000));
    }
}