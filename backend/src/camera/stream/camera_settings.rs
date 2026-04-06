/// Camera settings that can be adjusted from the UI.
/// Needs to be shared between main UI thread and background capture thread.
#[derive(Debug, Clone, Copy)]
pub struct CameraSettings {
    pub exposure_us: f64,
    pub frame_rate_hz: f64,
}

impl CameraSettings {
    pub fn new(exposure_us: f64, frame_rate_hz: f64) -> Self {
        Self {
            exposure_us,
            frame_rate_hz,
        }
    }
}
