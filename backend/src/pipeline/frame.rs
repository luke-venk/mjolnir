use opencv::core::Mat;
use opencv::prelude::MatTraitConstManual;
use std::sync::OnceLock;

/// The frame contains all the information that is needed associated with a specific
/// frame from a camera for computer vision.
///
/// Keep in mind that `Mat` variables should own their data as opposed to having
/// an unsafe mutable pointer to another data buffer.
///
/// For example: If a `Mat` variable is pointing to a Box<[u8]> which eventually
/// gets dropped, using the `Mat` would cause a use-after-free crash
///
/// Because of the unsafe operations that `Mat` performs, any operations using `Mat`
/// CANNOT modify the underlying data when that data is owned elsewhere.
///
/// For example: If we pass a pointer pointing to a Box<[u8]> to an unsafe `Mat`
/// constructor, and the constructor modifies that data, that's undefined behavior.
///
/// Note that `OnceLock<Mat>` is used to store the results of each pipeline stage.
/// Since each stage runs on its own thread, `OnceLock` makes these thread safe
/// while only being written once.
#[derive(Debug, Clone)]
pub struct Frame {
    /// The raw bytes for the frame coming from the camera at full resolution.
    raw_bytes_full_resolution: Box<[u8]>,

    /// The (width, height) of pixels for the frame. Note that this shouldn't
    /// be confused with (rows, cols), which is in fact the opposite but
    /// is the convention for matrices.
    raw_full_resolution: (i32, i32),

    /// The frame after lens undistortion is applied, before intensity normalization is applied.
    undistorted_image: OnceLock<Mat>,

    /// The frame after intensity normalization is applied, before downsampling is applied.
    intensity_normalized_image: OnceLock<Mat>,

    /// The frame after downsampling is applied, before Mog2 is applied.
    downsampled_image: OnceLock<Mat>,

    /// Frame metadata like timestamps.
    context: Context,
}

impl Frame {
    pub fn new(data: Box<[u8]>, resolution: (i32, i32), context: Context) -> Self {
        Self {
            raw_bytes_full_resolution: data,
            raw_full_resolution: resolution,
            undistorted_image: OnceLock::new(),
            intensity_normalized_image: OnceLock::new(),
            downsampled_image: OnceLock::new(),
            context,
        }
    }

    pub fn raw_bytes_full_resolution(&self) -> &Box<[u8]> {
        &self.raw_bytes_full_resolution
    }

    pub fn raw_full_resolution(&self) -> (i32, i32) {
        self.raw_full_resolution
    }

    pub fn undistorted_image(&self) -> Option<&Mat> {
        self.undistorted_image.get()
    }

    pub fn set_undistorted_image(&self, mat: Mat) -> Result<(), String> {
        self.undistorted_image
            .set(mat)
            .map_err(|_| "Undistorted image already set.".to_string())
    }

    pub fn intensity_normalized_image(&self) -> Option<&Mat> {
        self.intensity_normalized_image.get()
    }

    pub fn set_intensity_normalized_image(&self, mat: Mat) -> Result<(), String> {
        self.intensity_normalized_image
            .set(mat)
            .map_err(|_| "Intensity normalized image already set.".to_string())
    }

    pub fn downsampled_image(&self) -> Option<&Mat> {
        self.downsampled_image.get()
    }

    pub fn set_downsampled_image(&self, mat: Mat) -> Result<(), String> {
        self.downsampled_image
            .set(mat)
            .map_err(|_| "Downsampled normalized image already set.".to_string())
    }

    pub fn context(&self) -> Context {
        self.context
    }
}

/// Used for our pipeline stages to know which camera a given frame is from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CameraId {
    FieldLeft,
    FieldRight,
}

/// Our metadata for each frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Context {
    camera_id: CameraId,
    timestamp: u64,
}

impl Context {
    pub fn new(camera_id: CameraId, timestamp: u64) -> Self {
        Self {
            camera_id,
            timestamp,
        }
    }

    pub fn camera_id(&self) -> CameraId {
        self.camera_id
    }

    pub fn timestamp(&self) -> u64 {
        self.timestamp
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::AtlasATP124SResolution;
    use crate::pipeline::test_utils::{ComputerVisionStage, generate_frame};

    #[test]
    fn test_context_constructor_and_getters() {
        let context = Context::new(CameraId::FieldLeft, 6767);

        assert_eq!(context.camera_id(), CameraId::FieldLeft);
        assert_eq!(context.timestamp(), 6767);
    }

    #[test]
    fn test_frame_constructor_and_getters() {
        let frame: Frame = generate_frame(
            21,
            1342,
            AtlasATP124SResolution::Quarter,
            ComputerVisionStage::ForwardDownsampledCopy,
        );

        for &pixel in frame.raw_bytes_full_resolution() {
            assert_eq!(pixel, 21u8);
        }
        for pixel in frame.undistorted_image().unwrap().iter::<u8>().unwrap() {
            // Access 1st element because 0th is pixel coordinate and 2nd is value.
            assert_eq!(pixel.1, 21u8);
        }
        for pixel in frame
            .intensity_normalized_image()
            .unwrap()
            .iter::<u8>()
            .unwrap()
        {
            // Access 1st element because 0th is pixel coordinate and 2nd is value.
            assert_eq!(pixel.1, 21u8);
        }
        assert_eq!(frame.context().camera_id(), CameraId::FieldLeft);
        assert_eq!(frame.context().timestamp(), 1342);
    }
}
