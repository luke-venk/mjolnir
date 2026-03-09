use crate::schemas::{Frame, Context};

pub fn forward_downsampled_copy(frame: Frame) -> Frame {
    // TODO: implement actual logic
    
    let data = frame.data();
    let new_metadata = frame.context().metadata() + 1;
    Frame::new(data.to_vec(), Context::new(new_metadata))
}