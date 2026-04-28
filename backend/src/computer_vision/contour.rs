use crate::pipeline::Frame;

pub fn contour(mut frame: Frame) -> Frame {
    // TODO: implement actual contour logic.
    // For now, mark that contour ran, with no detection result.
    frame.context_mut().set_detected(Some(false));
    frame.context_mut().set_centroid(None, None);
    frame
}
