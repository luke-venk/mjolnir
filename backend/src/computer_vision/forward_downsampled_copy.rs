use crate::schemas::{Context, Frame};

pub fn forward_downsampled_copy(frame: Frame) -> Frame {
    // TODO: implement actual logic

    let data = frame.data();
    let new_timestamp = frame.context().timestamp() + 1;
    Frame::new(data.to_vec(), Context::new(new_timestamp))
}
