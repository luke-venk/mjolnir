use crate::pipeline::Frame;

/// The number of frames required for Mog2 to build the background model.
pub const MOG2_HISTORY_FRAMES: usize = 300;

pub fn mog2(frame: Frame) -> Frame {
    // TODO: Currently just passes the frame through this stage untouched.
    // Please implement the actual Mog2 logic.

    frame.clone()
}
