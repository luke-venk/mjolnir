use crate::pipeline::Frame;
use opencv::core::Mat;
use opencv::prelude::MatTraitConst;
use opencv::prelude::MatTraitConstManual;

pub fn undistortion(frame: Frame) -> Frame {
    let mat = Mat::new_rows_cols_with_data(
        frame.raw_full_resolution().1 as i32,
        frame.raw_full_resolution().0 as i32,
        frame.raw_bytes_full_resolution().as_ref(),
    )
    .expect("Failed to create Mat from raw bytes")
    .try_clone()
    .expect("Failed to clone Mat");

    frame
        .set_undistorted_image(mat)
        .expect("Failed to set undistorted image");
    frame
}
