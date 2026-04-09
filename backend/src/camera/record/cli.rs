use crate::camera::Resolution;

use clap::{ArgAction, Parser};

/// The command line arguments we'd expect for recording, regardless of whether
/// the user wishes to record with one, or record with both.
#[derive(Parser, Debug, Clone)]
#[command(name = "common_record_args")]
#[command(about = "Records Aravis camera frames into an output directory, optionally via per-frame lossless compression.")]
pub struct CommonRecordArgs {
    /// Whether to write losslessly compressed per-frame files instead of raw per-frame dumps.
    #[arg(long, action = ArgAction::Set, default_value_t = true)]
    pub compress: bool,

    /// Resolution of the footage.
    #[arg(long, value_enum, default_value_t = Resolution::UHD4K)]
    pub resolution: Resolution,
    
    /// Exposure time in microseconds.
    #[arg(long = "exposure-us", default_value_t = 10000.0)]
    pub exposure_time_us: f64,

    #[arg(long = "frame-rate-hz", default_value_t = 30.0)]
    pub frame_rate_hz: f64,

    /// Optional lens iris feature value. Only applied if the device exposes an Iris feature.
    #[arg(long)]
    pub aperture: Option<f64>,

    /// The number of buffers the cameras can push frames to, enabling asynchrony.
    #[arg(long, default_value_t = 8)]
    pub num_buffers: usize,

    /// Timeout for waiting on a frame buffer, in milliseconds.
    #[arg(long, default_value_t = 5000)]
    pub timeout_ms: u64,

    /// Output directory where frames will be written to.
    #[arg(long, visible_alias = "save-recordings-dir")]
    pub output_dir: String,

    /// Optional directory where compressed recordings will be recovered into PNG frames.
    #[arg(long)]
    pub recover_to_png_dir: Option<String>,

    /// Stop recording after this many frames per camera.
    #[arg(long)]
    pub max_frames: Option<usize>,

    /// Stop recording after this many seconds.
    #[arg(long)]
    pub max_duration: Option<f64>,

    /// Whether to enable Precision Time Protocol if supported by the device.
    #[arg(long, action = ArgAction::Set, default_value_t = true)]
    pub enable_ptp: bool,
}

impl CommonRecordArgs {
    /// Ensure that at least one stop condition was provided.
    pub fn validate(&self) -> Result<(), String> {
        if self.max_frames.is_none() && self.max_duration.is_none() {
            Err(
                "You must provide at least one stopping condition: --max-frames or --max-duration"
                    .to_string(),
            )
        } else if self.recover_to_png_dir.is_some() && !self.compress {
            Err(
                "recover_to_png_dir can only be used when --compress is enabled".to_string(),
            )
        } else {
            Ok(())
        }
    }
}

/// The args for recording with one camera should include exactly
/// which camera to record with.
#[derive(Parser, Debug, Clone)]
#[command(name = "record_from_one_camera_args")]
#[command(about = "Records frames from 1 Aravis camera into an output directory.")]
pub struct RecordWithOneCameraArgs {
    // Camera identifer. Aravis accepts device name, IP address, etc.
    #[arg(long = "camera", required = true)]
    pub camera_id: String,

    // Common record args.
    #[command(flatten)]
    pub common_args: CommonRecordArgs,
}

/// The args for recording with both cameras have no extra args.
#[derive(Parser, Debug, Clone)]
#[command(name = "record_from_both_camera_args")]
#[command(about = "Records frames from both Aravis cameras into an output directory.")]
pub struct RecordWithBothCamerasArgs {
    // Common record args.
    #[command(flatten)]
    pub common_args: CommonRecordArgs,
}
