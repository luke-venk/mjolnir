// Forward downsampled copy is the 2nd computer vision stage in the pipeline.
// In general, this means reducing an image's resolution, taking our 4k image
// and shrinking it to a smaller one. This also would entail converting our
// frames from color to grayscale, but our cameras are already monochrome.
#![allow(unused_imports)]
use crate::pipeline::Frame;
use opencv::core::{Mat, Size};
use opencv::imgproc::{resize, INTER_LINEAR};
use opencv::prelude::MatTraitConst;
use opencv::prelude::MatTraitConstManual;

const DOWNSAMPLED_WIDTH_PX: i32 = 960;
const DOWNSAMPLED_HEIGHT_PX: i32 = 540;
const SIZE: Size = Size::new(DOWNSAMPLED_WIDTH_PX, DOWNSAMPLED_HEIGHT_PX);

pub fn forward_downsampled_copy(frame: Frame) -> Frame {
    // Get the matrix acted on by the previous stage and define an output
    // matrix for the CV operation to write to.
    let input_mat = frame
        .undistorted_image()
        .expect("Error: Undistorted Mat hasn't been set yet.");
    let mut output_mat: Mat = Mat::default();

    // Perform resizing operation.
    if let Err(err) = resize(input_mat, &mut output_mat, SIZE, 0.0, 0.0, INTER_LINEAR) {
        eprintln!(
            "Error: Failed to downsample frame in forward_downsampled_copy(). Returning original frame. {err}"
        );
        return frame;
    }

    // Set the downsampled image to the result and return the frame.
    frame
        .set_downsampled_image(output_mat)
        .expect("Error: Failed to set DownsampledImage Mat.");
    frame
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::AtlasATP124SResolution;
    use crate::pipeline::test_utils::{generate_frame, ComputerVisionStage};
    use rstest::rstest;

    #[rstest]
    #[case(AtlasATP124SResolution::Quarter)]
    #[case(AtlasATP124SResolution::Half)]
    #[case(AtlasATP124SResolution::Full)]
    fn test_downsample_output_dimensions_and_values(#[case] resolution: AtlasATP124SResolution) {
        let input_frame: Frame = generate_frame(
            69,
            6969,
            resolution,
            ComputerVisionStage::ForwardDownsampledCopy,
        );
        let output_frame: Frame = forward_downsampled_copy(input_frame);
        let downsampled_mat: &Mat = output_frame.downsampled_image().unwrap();

        // Check dimensions match expected for downsampling.
        assert_eq!(downsampled_mat.rows(), DOWNSAMPLED_HEIGHT_PX);
        assert_eq!(downsampled_mat.cols(), DOWNSAMPLED_WIDTH_PX);

        // Check data stayed the same.
        for pixel in downsampled_mat.iter::<u8>().unwrap() {
            // Access 1st element because 0th is pixel coordinate and
            // 2nd is value.
            assert_eq!(pixel.1, 69u8);
        }
    }
}
