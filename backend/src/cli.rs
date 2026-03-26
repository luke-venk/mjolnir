use clap::{Args, Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "camera_utils")]
#[command(about = "Camera discovery and recording utilities")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// List all discoverable GenICam cameras on the network.
    DiscoverCameras,

    /// Open one or more cameras by discovered device id and start capture.
    RecordFromCameras(RecordFromCamerasArgs),
}

#[derive(Args, Debug, Clone)]
pub struct RecordFromCamerasArgs {
    #[arg(long = "camera", required = true)]
    pub cameras: Vec<String>,

    /// Exposure time in microseconds.
    #[arg(long = "exposure-us", default_value_t = 25.4)]
    pub exposure_us: f64,

    #[arg(long = "frame-rate-hz", default_value_t = 30.0)]
    pub frame_rate_hz: f64,

    /// Optional lens iris feature value. Only applied if the device exposes an Iris feature.
    #[arg(long)]
    pub aperture: Option<f64>,

    #[arg(long, default_value_t = 16)]
    pub num_buffers: usize,

    #[arg(long, default_value_t = 200)]
    pub timeout_ms: u64,

    #[arg(long, default_value_t = false)]
    pub use_fake_interface: bool,

    #[arg(long)]
    pub save_recordings_dir: Option<String>,
}
