// Helper functions to make testing with CV frames easier.
use super::{CameraId, Context, Frame};
use crate::camera::AtlasATP124SResolution;
use opencv::core::{CV_8U, Mat, Scalar};

#[derive(PartialEq, PartialOrd)]
pub enum ComputerVisionStage {
    Undistortion,
    IntensityNormalization,
    ForwardDownsampledCopy,
    Mog2,
    Contour,
}

/// Helper function to generate a frame.
///
/// `stage_to_prep_for` is used to populate the matrices up to that stage, since
/// the stages before the stage need to populate the matrices before, but the
/// current stage needs to have an empty `Mat` since `OnceLock` only lets populating
/// one time.
pub fn generate_frame(
    value: u8,
    timestamp: u64,
    camera_resolution: AtlasATP124SResolution,
    stage_to_prep_for: ComputerVisionStage,
) -> Frame {
    // Initialize frame.
    let resolution: (i32, i32) = camera_resolution.dimensions();
    let context: Context = Context::new(CameraId::FieldLeft, timestamp);
    let data: Box<[u8]> = vec![value; (resolution.0 * resolution.1) as usize].into_boxed_slice();
    let frame: Frame = Frame::new(data, resolution, context);

    // Set matrix placeholder values and populate frame matrices.
    let mat: Mat = Mat::new_rows_cols_with_default(
        resolution.1,
        resolution.0,
        CV_8U,
        Scalar::all(value as f64),
    )
    .unwrap();

    if stage_to_prep_for > ComputerVisionStage::Undistortion {
        frame.set_undistorted_image(mat.clone()).unwrap();
    }
    if stage_to_prep_for > ComputerVisionStage::IntensityNormalization {
        frame.set_intensity_normalized_image(mat.clone()).unwrap();
    }
    if stage_to_prep_for > ComputerVisionStage::ForwardDownsampledCopy {
        frame.set_downsampled_image(mat.clone()).unwrap();
    }

    frame
}
