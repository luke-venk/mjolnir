use crate::pipeline::Frame;

#[derive(Debug, Clone)]
pub struct MatchedFramePair {
    left: Frame,
    right: Frame,
}

impl MatchedFramePair {
    pub fn new(left: Frame, right: Frame) -> Self {
        Self { left, right }
    }

    pub fn left(&self) -> &Frame {
        &self.left
    }

    pub fn right(&self) -> &Frame {
        &self.right
    }

    pub fn pair_timestamp_ns(&self) -> u64 {
        let left_ts = self.left.context().camera_buffer_timestamp();
        let right_ts = self.right.context().camera_buffer_timestamp();
        (left_ts / 2) + (right_ts / 2) + ((left_ts % 2 + right_ts % 2) / 2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::{CameraId, Context};

    fn make_frame(
        camera_id: CameraId,
        timestamp_ns: u64,
        detected: Option<bool>,
        centroid: Option<(f64, f64)>,
    ) -> Frame {
        let mut frame = Frame::new(
            vec![1, 2, 3, 4].into_boxed_slice(),
            (2, 2),
            Context::new(camera_id, timestamp_ns),
        );
        frame.context_mut().set_detected(detected);
        frame.context_mut().set_centroid(centroid);
        frame
    }

    #[test]
    fn test_matched_frame_pair_constructor_and_getters() {
        let left = make_frame(CameraId::FieldLeft, 110, Some(true), Some((10.0, 20.0)));
        let right = make_frame(CameraId::FieldRight, 210, Some(false), None);

        let pair = MatchedFramePair::new(left, right);

        assert_eq!(pair.left().context().camera_buffer_timestamp(), 110);
        assert_eq!(pair.right().context().camera_buffer_timestamp(), 210);
        assert_eq!(pair.left().context().detected(), Some(true));
        assert_eq!(pair.right().context().detected(), Some(false));
        assert_eq!(pair.pair_timestamp_ns(), 160);
    }
}
