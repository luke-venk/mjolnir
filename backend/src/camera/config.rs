// Code for handling configurations for recording with Aravis.
use crate::camera::RecordWithBothCamerasArgs;
use crate::camera::RecordWithOneCameraArgs;
use crate::camera::StreamFromCamerasArgs;
use clap::ValueEnum;
use std::fmt;

/// Configuration for what specs we want to use while recording.
#[derive(Debug, Clone)]
pub struct CameraIngestConfig {
    pub camera_id: String,

    // Core capture settings.
    pub exposure_time_us: f64,
    pub frame_rate_hz: f64,
    pub resolution: AtlasATP124SResolution,

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

/// Different resolutions we might want to record with for the LUCID
/// Vision Labs Atlas ATP124S cameras.
/// See https://www.edmundoptics.com/p/lucid-vision-labst-atlas-atp124s-mc-sony-imx545-123mp-ip67-monochrome-camera/49821/.
#[derive(Debug, Clone, Copy, ValueEnum, Eq, PartialEq, Hash, Default)]
pub enum AtlasATP124SResolution {
    #[value(name = "quarter")]
    Quarter,

    #[value(name = "half")]
    Half,

    #[value(name = "full")]
    #[default]
    Full,
}

impl AtlasATP124SResolution {
    /// Note that the dimensions are width x height. This should not be confused
    /// with rows x cols, which is in fact the opposite.
    pub fn dimensions(&self) -> (u32, u32) {
        match self {
            AtlasATP124SResolution::Quarter => (1024, 750),
            AtlasATP124SResolution::Half => (2048, 1500),
            AtlasATP124SResolution::Full => (4096, 3000),
        }
    }

    pub fn binning(&self) -> i32 {
        match self {
            AtlasATP124SResolution::Quarter => 4,
            AtlasATP124SResolution::Half => 2,
            AtlasATP124SResolution::Full => 1,
        }
    }
}

impl fmt::Display for AtlasATP124SResolution {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AtlasATP124SResolution::Quarter => write!(f, "quarter"),
            AtlasATP124SResolution::Half => write!(f, "half"),
            AtlasATP124SResolution::Full => write!(f, "full"),
        }
    }
}
