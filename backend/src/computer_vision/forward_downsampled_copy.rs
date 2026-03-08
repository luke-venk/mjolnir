use crate::hardware::{Frame, Context};

pub fn forward_downsampled_copy(frame: Frame) -> Frame {
    // TODO: implement actual logic
    
    let data = frame.data();
    let new_metadata = frame.context().metadata() + 1;
    println!("Performing forward_downsampled_copy on frame with metadata: {:?}", new_metadata);  // TODO: remove
    Frame::new(data.to_vec(), Context::new(new_metadata))
}