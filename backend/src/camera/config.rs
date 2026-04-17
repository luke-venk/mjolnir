use crate::camera::record::cli::RecordWithBothCamerasArgs;
use crate::camera::record::cli::RecordWithOneCameraArgs;
use crate::camera::stream::cli::StreamFromCamerasArgs;
/// Code for handling configurations for recording with Aravis.
use clap::ValueEnum;

/// Configuration for what specs we want to use while recording.
#[derive(Debug, Clone)]
pub struct CameraIngestConfig {
    pub camera_id: String,

    // Core capture settings.
    pub exposure_time_us: f64,
    pub frame_rate_hz: f64,
    pub resolution: Resolution,

    // System-level config.
    pub num_buffers: usize,
    pub timeout_ms: u64,

    // Tool to request streaming restart if specifications are changed.
    pub restart_requested: bool,
}

impl CameraIngestConfig {
    pub fn from_record_one_args(args: RecordWithOneCameraArgs) -> Self {
        Self {
            camera_id: args.camera_id,
            exposure_time_us: args.common_args.exposure_time_us,
            frame_rate_hz: args.common_args.frame_rate_hz,
            resolution: args.common_args.resolution,
            num_buffers: args.common_args.num_buffers,
            timeout_ms: args.common_args.timeout_ms,
            restart_requested: false,
        }
    }

    pub fn from_record_both_args(camera_id: String, args: RecordWithBothCamerasArgs) -> Self {
        Self {
            camera_id,
            exposure_time_us: args.common_args.exposure_time_us,
            frame_rate_hz: args.common_args.frame_rate_hz,
            resolution: args.common_args.resolution,
            num_buffers: args.common_args.num_buffers,
            timeout_ms: args.common_args.timeout_ms,
            restart_requested: false,
        }
    }

    pub fn from_stream_args(args: StreamFromCamerasArgs) -> Self {
        Self {
            camera_id: args.camera_id,
            exposure_time_us: args.exposure_time_us,
            frame_rate_hz: args.frame_rate_hz,
            resolution: args.resolution,
            num_buffers: 8,
            timeout_ms: 5000,
            restart_requested: false,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.camera_id.is_empty() {
            return Err("camera_id cannot be empty".to_string());
        }
        if self.exposure_time_us <= 0.0 {
            return Err("exposure_time_us must be > 0".to_string());
        }
        if self.frame_rate_hz <= 0.0 {
            return Err("frame_rate_hz must be > 0".to_string());
        }
        if self.num_buffers == 0 {
            return Err("num_buffers must be > 0".to_string());
        }
        Ok(())
    }
}

/// Different resolutions we might want to record with.
#[derive(Debug, Clone, Copy, ValueEnum, PartialEq)]
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
        // Check https://www.edmundoptics.com/p/lucid-vision-labst-atlas-atp124s-mc-sony-imx545-123mp-ip67-monochrome-camera/49821/.
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

    pub fn to_string(&self) -> String {
        match self {
            Resolution::HD => "720p".to_string(),
            Resolution::FullHD => "1080p".to_string(),
            Resolution::UHD4K => "4k".to_string(),
        }
    }
}
