use clap::Parser;
use crate::camera::Resolution;

/// The command line arguments we'd expect for the cameras to live stream.
#[derive(Parser, Debug, Clone)]
#[command(name = "live_stream")]
#[command(about = "Streams raw frames from Aravis camera to laptop UI.")]
pub struct StreamFromCamerasArgs {
    #[arg(long = "camera", required = true)]
    pub camera_id: String,

    // Resolution to start with.
    #[arg(long, value_enum, default_value_t = Resolution::HD)]
    pub resolution: Resolution,
    
    // Exposure time to start with (microseconds).
    #[arg(long = "exposure-us", default_value_t = 10000.0)]
    pub exposure_time_us: f64,

    // Frame rate to start with.
    #[arg(long = "frame-rate-hz", default_value_t = 15.0)]
    pub frame_rate_hz: f64,
}
