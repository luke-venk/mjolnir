use crate::schemas::CameraId;
use clap::ValueEnum;

#[derive(Debug, Clone)]
pub struct CameraIngestConfig {
    pub device_id: String,
    pub exposure_time_us: f64,
    pub frame_rate_hz: f64,
    pub resolution: Resolution,
    pub aperture: Option<f64>,
    pub enable_ptp: bool,
    pub use_fake_interface: bool,
    pub num_buffers: usize,
    pub timeout_ms: u64,
    pub max_frames: Option<usize>,
    pub max_duration_s: Option<f64>,
}

impl CameraIngestConfig {
    pub fn new(_camera_id: CameraId) -> Self {
        Self {
            device_id: String::new(),
            exposure_time_us: 25.4,
            frame_rate_hz: 30.0,
            resolution: Resolution::UHD4K,
            aperture: None,
            enable_ptp: false,
            use_fake_interface: true,
            num_buffers: 16,
            timeout_ms: 200,
            max_frames: None,
            max_duration_s: None,
        }
    }

    pub fn validate(self) -> Result<Self, String> {
        if self.device_id.trim().is_empty() && !self.use_fake_interface {
            return Err("device_id cannot be empty unless use_fake_interface=true".to_string());
        }

        if self.exposure_time_us < 25.4 || self.exposure_time_us > 10_000_000.0 {
            return Err(format!(
                "exposure_time_us must be between 25.4 and 10,000,000.0 microseconds, got {}",
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

        if let Some(max_frames) = self.max_frames {
            if max_frames == 0 {
                return Err("max_frames must be > 0 when provided".to_string());
            }
        }

        if let Some(max_duration_s) = self.max_duration_s {
            if max_duration_s <= 0.0 {
                return Err("max_duration_s must be > 0 when provided".to_string());
            }
        }

        Ok(self)
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Resolution {
    #[value(name = "720p")]
    HD,
    #[value(name = "1080p")]
    FullHD,
    #[value(name = "4k")]
    UHD4K,
}

impl Resolution {
    pub fn dimensions(&self) -> (i32, i32) {
        match self {
            Resolution::HD => (1024, 750),
            Resolution::FullHD => (2048, 1500),
            Resolution::UHD4K => (4096, 3000),
        }
    }

    pub fn binning(&self) -> i32 {
        match self {
            Resolution::HD => 4,
            Resolution::FullHD => 2,
            Resolution::UHD4K => 1,
        }
    }
}
