use crate::camera::Resolution;

use clap::Parser;

/// The command line arguments we'd expect for the cameras to record.
#[derive(Parser, Debug, Clone)]
#[command(name = "record_from_cameras")]
#[command(about = "Records raw frames from Aravis camera(s) into an output directory.")]
pub struct RecordFromCamerasArgs {
    #[arg(long = "camera", required = true)]
    pub camera_id: String,

    /// Exposure time in microseconds.
    #[arg(long = "exposure-us", default_value_t = 100.0)]
    pub exposure_us: f64,

    #[arg(long = "frame-rate-hz", default_value_t = 30.0)]
    pub frame_rate_hz: f64,

    #[arg(long, value_enum, default_value_t = Resolution::UHD4K)]
    pub resolution: Resolution,

    /// Optional lens iris feature value. Only applied if the device exposes an Iris feature.
    #[arg(long)]
    pub aperture: Option<f64>,

    /// The number of buffers the cameras can push frames to, enabling asynchrony.
    #[arg(long, default_value_t = 64)]
    pub num_buffers: usize,

    /// Timeout for waiting on a frame buffer, in milliseconds.
    #[arg(long, default_value_t = 5000)]
    pub timeout_ms: u64,

    /// Output directory where frames will be written to.
    #[arg(long)]
    pub output_dir: String,

    /// Stop recording after this many frames per camera.
    #[arg(long)]
    pub max_frames: Option<usize>,

    /// Stop recording after this many seconds.
    #[arg(long)]
    pub max_duration: Option<f64>,

    /// Whether to enable Precision Time Protocol if supported by the device.
    #[arg(long, default_value_t = false)]
    pub enable_ptp: bool,
}

impl RecordFromCamerasArgs {
    pub fn validate(&self) -> Result<(), String> {
        // Ensure that at least one stop condition was provided.
        if self.max_frames.is_none() && self.max_duration.is_none() {
            Err(
                "You must provide at least one stopping condition: --max-frames or --max-duration"
                    .to_string(),
            )
        } else {
            Ok(())
        }
    }
}
