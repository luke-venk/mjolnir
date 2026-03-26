use crate::schemas::CameraId;

#[derive(Debug, Clone)]
pub struct CameraIngestConfig {
    pub device_id: String,
    pub exposure_time_us: f64,
    pub frame_rate_hz: f64,
    pub aperture: Option<f64>,
    pub enable_ptp: bool,
    pub use_fake_interface: bool,
    pub num_buffers: usize,
    pub timeout_ms: u64,
}

impl CameraIngestConfig {
    //Temp constructor for pipeline starting
    //Gives pipelien starting config object
    pub fn new(camera_id: CameraId) -> Self {
        let _camera_id = camera_id; 
        Self {
            device_id: String::new(),
            exposure_time_us: 25.4,
            frame_rate_hz: 30.0,
            aperture: None, 
            enable_ptp: false, 
            use_fake_interface: true, 
            num_buffers: 16,
            timeout_ms: 200,
        }
    }


    pub fn validate(self) -> Result<Self, String> {
        if self.device_id.trim().is_empty() && !self.use_fake_interface {
            return Err("camera_id cannot be empty unless use_fake_interface=true".to_string());
        }

        // Per review feedback: support full 25.4 us to 10 s range.
        if self.exposure_time_us < 25.4 || self.exposure_time_us > 10_000_000.0 {
            return Err(format!(
                "exposure_us must be between 25.4 and 10,000,000.0 microseconds, got {}",
                self.exposure_time_us
            ));
        }

        if self.frame_rate_hz <= 0.0 {
            return Err(format!(
                "frame_rate_hz must be > 0, got {}",
                self.frame_rate_hz
            ));
        }

        if self.num_buffers == 0 {
            return Err("num_buffers must be > 0".to_string());
        }

        if self.timeout_ms == 0 {
            return Err("timeout_ms must be > 0".to_string());
        }

        Ok(self)
    }
}
