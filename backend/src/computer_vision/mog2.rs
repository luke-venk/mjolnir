use crate::schemas::{Context, Frame};

/// The number of frames required for Mog2 to build the background model.
pub const MOG2_HISTORY_FRAMES: usize = 300;

pub fn mog2(frame: Frame) -> Frame {
    // TODO: implement actual logic

    let data = frame.data();
    let new_timestamp = frame.context().timestamp() + 1;
    Frame::new(data.to_vec(), Context::new(new_timestamp))
}
