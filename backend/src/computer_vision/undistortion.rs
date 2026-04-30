use crate::pipeline::Frame;
use opencv::core::{CV_8U, Mat, Scalar};
use opencv::prelude::*;

pub fn undistortion(frame: Frame) -> Frame {
    let (width, height) = frame.raw_full_resolution();

    // Allocate an owned Mat large enough to hold the full-resolution monochrome frame.
    let mut mat = match Mat::new_rows_cols_with_default(
        height as i32,
        width as i32,
        CV_8U,
        Scalar::all(0.0),
    ) {
        Ok(m) => m,
        Err(err) => {
            eprintln!("Error: Failed to allocate Mat in undistortion(). Returning original frame. {err}");
            return frame;
        }
    };

    // Copy raw camera bytes into the Mat so it owns its data.
    match mat.data_bytes_mut() {
        Ok(bytes) => bytes.copy_from_slice(frame.raw_bytes_full_resolution()),
        Err(err) => {
            eprintln!("Error: Failed to access Mat bytes in undistortion(). Returning original frame. {err}");
            return frame;
        }
    }

    // TODO: Apply lens undistortion once camera calibration (intrinsic matrix +
    // distortion coefficients) is available. Use imgproc::undistort() or
    // precompute remap tables with calib3d::init_undistort_rectify_map().

    frame
        .set_undistorted_image(mat)
        .expect("undistorted_image should not be set before the undistortion stage");
    frame
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::AtlasATP124SResolution;
    use crate::pipeline::test_utils::{ComputerVisionStage, generate_frame};
    use rstest::rstest;

    #[rstest]
    #[case(AtlasATP124SResolution::Quarter)]
    #[case(AtlasATP124SResolution::Half)]
    #[case(AtlasATP124SResolution::Full)]
    fn test_undistortion_sets_mat_with_correct_dimensions(
        #[case] resolution: AtlasATP124SResolution,
    ) {
        let frame = generate_frame(42, 1234, resolution, ComputerVisionStage::Undistortion);
        let output = undistortion(frame);

        let mat = output
            .undistorted_image()
            .expect("undistorted_image should be set after undistortion stage");
        let (width, height) = resolution.dimensions();

        assert_eq!(mat.cols(), width as i32);
        assert_eq!(mat.rows(), height as i32);
    }

    #[rstest]
    #[case(AtlasATP124SResolution::Quarter)]
    #[case(AtlasATP124SResolution::Half)]
    #[case(AtlasATP124SResolution::Full)]
    fn test_undistortion_preserves_pixel_values(#[case] resolution: AtlasATP124SResolution) {
        let frame = generate_frame(99, 5678, resolution, ComputerVisionStage::Undistortion);
        let output = undistortion(frame);

        let mat = output
            .undistorted_image()
            .expect("undistorted_image should be set after undistortion stage");

        for pixel in mat.iter::<u8>().unwrap() {
            assert_eq!(pixel.1, 99u8);
        }
    }
}
