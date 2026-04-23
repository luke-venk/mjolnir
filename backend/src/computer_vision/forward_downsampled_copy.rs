// Forward downsampled copy is the 3rd computer vision stage in the pipeline.
// In general, this means reducing an image's resolution, taking our 4k image
// and shrinking it to a smaller one. This also would entail converting our
// frames from color to grayscale, but our cameras are already monochrome.
use crate::camera::Resolution;
use crate::pipeline::{Context, Frame};
use opencv::core::{CV_8U, Mat, Size};
use opencv::imgproc::{INTER_LINEAR, resize};

const DOWNSAMPLED_WIDTH_PX: i32 = 960;
const DOWNSAMPLED_HEIGHT_PX: i32 = 540;
const SIZE: Size = Size::new(DOWNSAMPLED_WIDTH_PX, DOWNSAMPLED_HEIGHT_PX);

pub fn forward_downsampled_copy(frame: Frame) -> Frame {
    // Resizing requires knowing the dimensions (in pixels) of the input frame.
    let resolution: Resolution = frame.context().resolution();
    let (original_height_px, original_width_px): (i32, i32) = resolution.dimensions();

    // OpenCV's resize function operates on Mat objects. So convert the frame's data
    // into this object, and define the output Mat which will store the resized object.
    let input_mat: Mat = unsafe {
        Mat::new_rows_cols_with_data_unsafe(
            original_height_px,
            original_width_px,
            CV_8U,
            frame.data_as_arr().as_ptr() as *mut _,
            opencv::core::Mat_AUTO_STEP,
        )
        .expect("Error: Failed to generate input Mat from frame in forward_downsampled_copy().")
    };
    let mut output_mat: Mat = Mat::default();

    if let Err(err) = resize(&input_mat, &mut output_mat, SIZE, 0.0, 0.0, INTER_LINEAR) {
        eprintln!(
            "Error: Failed to downsample frame in forward_downsampled_copy(). Returning original frame. {err}"
        );
        return frame;
    }

    // Update context's resolution to this processed resolution.
    let output_context: Context =
        Context::new(frame.context().timestamp(), Resolution::Downsampled);

    Frame::new(output_mat, output_context)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use crate::pipeline::test_utils::generate_frame;

    #[rstest]
    #[case(Resolution::HD)]
    #[case(Resolution::FullHD)]
    #[case(Resolution::UHD4K)]
    fn test_downsample_output_dimensions(#[case] resolution: Resolution) {
        let input_frame: Frame = generate_frame(69, 6969, resolution);

        let output_frame = forward_downsampled_copy(input_frame);

        // Check downsampled dimension and resolution.
        let expected_size: usize = (DOWNSAMPLED_WIDTH_PX * DOWNSAMPLED_HEIGHT_PX) as usize;
        assert_eq!(output_frame.data_as_arr().len(), expected_size);
        assert_eq!(output_frame.context().resolution(), Resolution::Downsampled);

        // Check data stayed the same.
        for &pixel in output_frame.data_as_arr().iter() {
            assert_eq!(pixel, 69u8);
        }
    }
}
