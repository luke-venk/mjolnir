use crate::pipeline::Frame;

pub fn undistortion(frame: Frame) -> Frame {
    // TODO: Currently just passes the frame through this stage untouched.
    // Please implement the actual undistortion logic.

    frame.clone()
}
