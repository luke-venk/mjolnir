use clap::Parser;
use std::path::PathBuf;

/// CLI arguments for the real backend binary (`prod_real_cameras` /
/// `dev_real_cameras`) when running against recorded footage.
#[derive(Parser, Debug, Clone)]
#[command(name = "real_backend_args")]
#[command(about = "Runs the real backend against a recorded footage session.")]
pub struct RealBackendArgs {
    /// Replay a session directory produced by `//backend:record`. The directory
    /// is expected to contain `left_cam/` and `right_cam/` subdirectories,
    /// each holding the per-frame `.tiff` + `.json` pair the recorder writes.
    #[arg(long = "feed-footage-dir")]
    pub feed_footage_dir: Option<PathBuf>,

    /// Exposure time in microseconds.
    #[arg(long = "exposure-us", default_value_t = 10000.0)]
    pub exposure_time_us: f64,

    /// Left camera id (figure this out with the :discover binary)
    #[arg(long = "left-camera-id", required = true)]
    pub left_camera_id: String,

    /// Right camera id (figure this out with the :discover binary)
    #[arg(long = "right-camera-id", required = true)]
    pub right_camera_id: String,
}

pub fn parse_real_backend_args() -> RealBackendArgs {
    RealBackendArgs::parse()
}
