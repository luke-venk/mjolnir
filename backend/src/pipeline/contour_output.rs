use crate::pipeline::{CameraId, Context, Frame};

#[derive(Debug, Clone, PartialEq)]
pub struct PixelCenter {
    cx_px: f64,
    cy_px: f64,
}

impl PixelCenter {
    pub fn new(cx_px: f64, cy_px: f64) -> Self {
        Self { cx_px, cy_px }
    }

    pub fn cx_px(&self) -> f64 {
        self.cx_px
    }

    pub fn cy_px(&self) -> f64 {
        self.cy_px
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ContourOutput {
    camera_id: CameraId,
    camera_buffer_timestamp: u64,
    detected: bool,
    center_px: Option<PixelCenter>,
}

impl ContourOutput {
    pub fn new(
        camera_id: CameraId,
        camera_buffer_timestamp: u64,
        detected: bool,
        center_px: Option<PixelCenter>,
    ) -> Self {
        Self {
            camera_id,
            camera_buffer_timestamp,
            detected,
            center_px,
        }
    }

    pub fn camera_id(&self) -> CameraId {
        self.camera_id
    }

    pub fn camera_buffer_timestamp(&self) -> u64 {
        self.camera_buffer_timestamp
    }

    pub fn center_px(&self) -> Option<&PixelCenter> {
        self.center_px.as_ref()
    }

    pub fn detected(&self) -> bool {
        self.detected
    }
}

impl From<Frame> for ContourOutput {
    fn from(frame: Frame) -> Self {
        let contour_output = Self::new(
            frame.context().camera_id(),
            frame.context().camera_buffer_timestamp(),
            frame.context().detected().unwrap_or(false),
            frame
                .context()
                .centroid()
                .map(|(cx, cy)| PixelCenter::new(cx, cy)),
        );

        let _ = frame.clear_undistorted_image();
        let _ = frame.clear_downsampled_image();

        contour_output
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatchedContourPair {
    left: ContourOutput,
    right: ContourOutput,
}

impl MatchedContourPair {
    pub fn new(left: ContourOutput, right: ContourOutput) -> Self {
        Self { left, right }
    }

    pub fn left(&self) -> &ContourOutput {
        &self.left
    }

    pub fn right(&self) -> &ContourOutput {
        &self.right
    }

    pub fn pair_timestamp_ns(&self) -> u64 {
        let left_ts = self.left.camera_buffer_timestamp();
        let right_ts = self.right.camera_buffer_timestamp();
        (left_ts / 2) + (right_ts / 2) + ((left_ts % 2 + right_ts % 2) / 2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::CameraId;

    #[test]
    fn test_contour_output_detected_when_center_present() {
        let output = ContourOutput::new(
            CameraId::FieldLeft,
            2000,
            true,
            Some(PixelCenter::new(123.0, 456.0)),
        );

        assert!(output.detected());
        let center = output.center_px().expect("expected center");
        assert_eq!(center.cx_px(), 123.0);
        assert_eq!(center.cy_px(), 456.0);
    }

    #[test]
    fn test_contour_output_not_detected_when_center_absent() {
        let output = ContourOutput::new(CameraId::FieldRight, 2001, false, None);

        assert!(!output.detected());
        assert!(output.center_px().is_none());
    }

    #[test]
    fn test_matched_contour_pair_constructor_and_getters() {
        let left = ContourOutput::new(
            CameraId::FieldLeft,
            110,
            true,
            Some(PixelCenter::new(10.0, 20.0)),
        );
        let right = ContourOutput::new(CameraId::FieldRight, 210, false, None);

        let pair = MatchedContourPair::new(left, right);

        assert_eq!(pair.left().camera_id(), CameraId::FieldLeft);
        assert_eq!(pair.right().camera_id(), CameraId::FieldRight);
        assert!(pair.left().detected());
        assert!(!pair.right().detected());
        assert_eq!(pair.pair_timestamp_ns(), 160);
    }

    #[test]
    fn test_contour_output_from_frame_preserves_context() {
        let mut frame = Frame::new(
            vec![1, 2, 3, 4].into_boxed_slice(),
            (2, 2),
            Context::new(CameraId::FieldLeft, 99),
        );
        frame.context_mut().set_detected(Some(true));
        frame.context_mut().set_centroid(Some((7.0, 8.0)));

        frame.set_undistorted_image(opencv::core::Mat::default()).unwrap();
        frame.set_downsampled_image(opencv::core::Mat::default()).unwrap();

        let output = ContourOutput::from(frame);
        assert!(output.detected());
        let center = output.center_px().expect("expected center");
        assert_eq!(center.cx_px(), 7.0);
        assert_eq!(center.cy_px(), 8.0);
    }
}
