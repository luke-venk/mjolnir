use crate::schemas::Context;

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
    context: Context,
    center_px: Option<PixelCenter>,
}

impl ContourOutput {
    pub fn new(context: Context, center_px: Option<PixelCenter>) -> Self {
        Self { context, center_px }
    }

    pub fn context(&self) -> &Context {
        &self.context
    }

    pub fn center_px(&self) -> Option<&PixelCenter> {
        self.center_px.as_ref()
    }

    pub fn detected(&self) -> bool {
        self.center_px.is_some()
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
        let left_ts = self.left.context().buffer_timestamp_ns();
        let right_ts = self.right.context().buffer_timestamp_ns();
        (left_ts / 2) + (right_ts / 2) + ((left_ts % 2 + right_ts % 2) / 2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schemas::CameraId;

    #[test]
    fn test_contour_output_detected_when_center_present() {
        let output = ContourOutput::new(
            Context::new(CameraId::FieldLeft, 3, 1000, 2000),
            Some(PixelCenter::new(123.0, 456.0)),
        );

        assert!(output.detected());
        let center = output.center_px().expect("expected center");
        assert_eq!(center.cx_px(), 123.0);
        assert_eq!(center.cy_px(), 456.0);
    }

    #[test]
    fn test_contour_output_not_detected_when_center_absent() {
        let output = ContourOutput::new(Context::new(CameraId::FieldRight, 4, 1001, 2001), None);

        assert!(!output.detected());
        assert!(output.center_px().is_none());
    }

    #[test]
    fn test_matched_contour_pair_constructor_and_getters() {
        let left = ContourOutput::new(
            Context::new(CameraId::FieldLeft, 1, 100, 110),
            Some(PixelCenter::new(10.0, 20.0)),
        );
        let right = ContourOutput::new(
            Context::new(CameraId::FieldRight, 2, 200, 210),
            None,
        );

        let pair = MatchedContourPair::new(left, right);

        assert_eq!(pair.left().context().camera_id(), CameraId::FieldLeft);
        assert_eq!(pair.right().context().camera_id(), CameraId::FieldRight);
        assert!(pair.left().detected());
        assert!(!pair.right().detected());
        assert_eq!(pair.pair_timestamp_ns(), 160);
    }
}
