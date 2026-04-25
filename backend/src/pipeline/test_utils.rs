// Helper functions to make testing with CV frames easier.
use crate::camera::Resolution;
use crate::pipeline::{Context, Frame};
use opencv::core::{CV_8U, Mat};

// Helper function to generate a frame.
pub fn generate_frame(value: u8, timestamp: u64, resolution: Resolution) -> Frame {
    let (rows, cols): (i32, i32) = resolution.dimensions();
    let context: Context = Context::new(timestamp, resolution);
    let data: Vec<u8> = vec![value; (rows * cols) as usize];
    let mat = unsafe {
        Mat::new_rows_cols_with_data_unsafe(
            rows,
            cols,
            CV_8U,
            data.as_ptr() as *mut _,
            opencv::core::Mat_AUTO_STEP,
        )
        .expect("Error: Failed to generate input Mat from frame in forward_downsampled_copy().")
    };
    Frame::new(mat, context)
}
