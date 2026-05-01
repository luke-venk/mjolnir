// Lens undistortion is the 1st stage in the pipeline. Immediately upon
// receiving a frame from the camera, we should correct for the radial
// and tangential distortion that lenses create, which cause straight
// lines to appear curved. This involves camera calibration to identify
// internal parameters, which is done on the field using ChArUcos boards.
//
// More information can be found at the following link:
// https://docs.opencv.org/4.x/dc/dbb/tutorial_py_calibration.html
use crate::pipeline::Frame;
use opencv::calib3d::undistort;
use opencv::core::{Mat, no_array};
use opencv::prelude::MatTraitConst;
use std::sync::OnceLock;

// The following are internal parameters that we determined via calibration
// for camera intrinsics and extrinsics. Note that I'm using OnceLock here
// to avoid reconstruting the matrices every time undistortion occurs. We
// must use the `Mat` type in order to pass these into the OpenCV `undistort`
// binding. We can't simply declare the `Mat` as const since it is heap-allocated.

// Input camera matrix (focal length, principal point, etc.).
static CAMERA_MATRIX: OnceLock<Mat> = OnceLock::new();

// Input vector of distortion coefficients (radial and tangential terms).
static DISTORTION_COEFFICIENTS: OnceLock<Mat> = OnceLock::new();

// TODO: update values.
fn camera_matrix() -> &'static Mat {
    CAMERA_MATRIX.get_or_init(|| {
        Mat::from_slice_2d(&[[69.0, 0.0, 69.0], [0.0, 69.0, 67.0], [0.0, 0.0, 1.0]])
            .expect("Failed to get camera matrix.")
    })
}

// TODO: update values.
fn distortion_coefficients() -> &'static Mat {
    DISTORTION_COEFFICIENTS.get_or_init(|| {
        Mat::from_slice(&[69.0_f64, 69.0, 69.0, 69.0, 69.0, 69.0, 69.0, 69.0])
            .expect("Failed to get distortion coefficients.")
            .try_clone()
            .expect("Failed to clone distortion coefficients.")
    })
}

pub fn undistortion(frame: Frame) -> Frame {
    // First, convert the raw bytes of the frame into an input OpenCV
    // matrix type, and create the output matrix to write to.
    let (cols, rows): (u32, u32) = frame.raw_full_resolution();
    let input_mat =
        Mat::new_rows_cols_with_data(rows as i32, cols as i32, frame.raw_bytes_full_resolution())
            .expect("Failed to create input matrix during lens undistortion.");
    let mut output_mat: Mat = Mat::default();

    // Then, perform the undistortion using OpenCV bindings.
    if let Err(err) = undistort(
        &input_mat,
        &mut output_mat,
        camera_matrix(),
        distortion_coefficients(),
        &no_array(),
    ) {
        eprintln!("Error: Failed to undistort in undistort(). Returning original frame. {err}");
        return frame;
    }

    // Set the undisorted image to the result and return the frame.
    frame
        .set_undistorted_image(output_mat)
        .expect("Error: Failed to set undistorted Mat.");
    frame
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        camera::AtlasATP124SResolution,
        pipeline::test_utils::{ComputerVisionStage, generate_frame},
    };
    use rstest::rstest;

    #[rstest]
    #[case(AtlasATP124SResolution::Quarter)]
    #[case(AtlasATP124SResolution::Half)]
    #[case(AtlasATP124SResolution::Full)]
    fn test_undistortion_acts_on_frame(#[case] resolution: AtlasATP124SResolution) {
        let input_frame: Frame =
            generate_frame(200, 4372, resolution, ComputerVisionStage::Undistortion);
        let output_frame: Frame = undistortion(input_frame);

        // Check that output exists and that its dimensions match input dimensions.
        let undistorted_mat: &Mat = output_frame.undistorted_image().unwrap();
        assert_eq!(
            undistorted_mat.rows(),
            output_frame.raw_full_resolution().1 as i32
        );
        assert_eq!(
            undistorted_mat.cols(),
            output_frame.raw_full_resolution().0 as i32
        );
    }
}
