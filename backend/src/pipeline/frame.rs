use opencv::core::Mat;
use opencv::prelude::MatTraitConstManual;
use std::sync::RwLock;

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
/// Note that `RwLock<Option<Mat>>` is used to store the results of each pipeline
/// stage. This keeps stage outputs thread safe while also allowing late-stage
/// cleanup to drop image buffers once a slim downstream artifact has been built.
#[derive(Debug, Clone)]
pub struct Frame {
    /// The raw bytes for the frame coming from the camera at full resolution.
    raw_bytes_full_resolution: Option<Box<[u8]>>,

    /// The (width, height) of pixels for the frame. Note that this shouldn't
    /// be confused with (rows, cols), which is in fact the opposite but
    /// is the convention for matrices.
    raw_full_resolution: (u32, u32),

    /// The frame after lens undistortion is applied, before downsampling is applied.
    undistorted_image: RwLock<Option<Mat>>,

    /// The frame after downsampling is applied, before Mog2 is applied.
    downsampled_image: RwLock<Option<Mat>>,

    /// Frame metadata like timestamps.
    context: Context,
}

impl Frame {
    pub fn new(data: Box<[u8]>, resolution: (u32, u32), context: Context) -> Self {
        Self {
            raw_bytes_full_resolution: Some(data),
            raw_full_resolution: resolution,
            undistorted_image: RwLock::new(None),
            downsampled_image: RwLock::new(None),
            context,
        }
    }

    pub fn raw_bytes_full_resolution(&self) -> Option<&Box<[u8]>> {
        self.raw_bytes_full_resolution.as_ref()
    }

    pub fn clear_raw_bytes_full_resolution(&mut self) {
        self.raw_bytes_full_resolution = None;
    }

    pub fn raw_full_resolution(&self) -> (u32, u32) {
        self.raw_full_resolution
    }

    pub fn undistorted_image(&self) -> Option<Mat> {
        self.undistorted_image
            .read()
            .ok()
            .and_then(|guard| guard.clone())
    }

    pub fn set_undistorted_image(&self, mat: Mat) -> Result<(), String> {
        let mut guard = self
            .undistorted_image
            .write()
            .map_err(|_| "Undistorted image lock poisoned.".to_string())?;
        if guard.is_some() {
            return Err("Undistorted image already set.".to_string());
        }
        *guard = Some(mat);
        Ok(())
    }

    pub fn clear_undistorted_image(&self) -> Result<(), String> {
        let mut guard = self
            .undistorted_image
            .write()
            .map_err(|_| "Undistorted image lock poisoned.".to_string())?;
        *guard = None;
        Ok(())
    }

    pub fn downsampled_image(&self) -> Option<Mat> {
        self.downsampled_image
            .read()
            .ok()
            .and_then(|guard| guard.clone())
    }

    pub fn set_downsampled_image(&self, mat: Mat) -> Result<(), String> {
        let mut guard = self
            .downsampled_image
            .write()
            .map_err(|_| "Downsampled image lock poisoned.".to_string())?;
        if guard.is_some() {
            return Err("Downsampled normalized image already set.".to_string());
        }
        *guard = Some(mat);
        Ok(())
    }

    pub fn clear_downsampled_image(&self) -> Result<(), String> {
        let mut guard = self
            .downsampled_image
            .write()
            .map_err(|_| "Downsampled image lock poisoned.".to_string())?;
        *guard = None;
        Ok(())
    }

    pub fn context(&self) -> &Context {
        &self.context
    }

    pub fn context_mut(&mut self) -> &mut Context {
        &mut self.context
    }
}

/// Used for our pipeline stages to know which camera a given frame is from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CameraId {
    FieldLeft,
    FieldRight,
}

/// Our metadata for each frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Context {
    camera_id: CameraId,

    /// The timestamp given to the buffer by the camera (i.e. buffer.timestamp()).
    camera_buffer_timestamp: u64,

    detected: Option<bool>,
    centroid: Option<(f64, f64)>,
}

impl Context {
    pub fn new(camera_id: CameraId, camera_buffer_timestamp: u64) -> Self {
        Self {
            camera_id,
            camera_buffer_timestamp,
            detected: None,
            centroid: None,
        }
    }

    pub fn camera_id(&self) -> CameraId {
        self.camera_id
    }

    pub fn camera_buffer_timestamp(&self) -> u64 {
        self.camera_buffer_timestamp
    }

    pub fn detected(&self) -> Option<bool> {
        self.detected
    }

    pub fn centroid(&self) -> Option<(f64, f64)> {
        self.centroid
    }

    pub fn set_detected(&mut self, detected: Option<bool>) {
        self.detected = detected;
    }

    pub fn set_centroid(&mut self, centroid: Option<(f64, f64)>) {
        self.centroid = centroid;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::AtlasATP124SResolution;
    use crate::pipeline::test_utils::{generate_frame, ComputerVisionStage};
    use rstest::rstest;

    #[rstest]
    fn test_context_constructor_and_getters() {
        let context = Context::new(CameraId::FieldLeft, 6767);

        assert_eq!(context.camera_id(), CameraId::FieldLeft);
        assert_eq!(context.camera_buffer_timestamp(), 6767);
        assert_eq!(context.detected(), None);
        assert_eq!(context.centroid(), None);
    }

    #[rstest]
    fn test_frame_constructor_and_getters() {
        let frame: Frame = generate_frame(
            21,
            1342,
            AtlasATP124SResolution::Quarter,
            ComputerVisionStage::ForwardDownsampledCopy,
        );

        for &pixel in frame.raw_bytes_full_resolution().unwrap() {
            assert_eq!(pixel, 21u8);
        }
        for pixel in frame.undistorted_image().unwrap().iter::<u8>().unwrap() {
            assert_eq!(pixel.1, 21u8);
        }
        assert_eq!(frame.context().camera_id(), CameraId::FieldLeft);
        assert_eq!(frame.context().camera_buffer_timestamp(), 1342);
        assert_eq!(frame.context().detected(), None);
        assert_eq!(frame.context().centroid(), None);
    }
}
