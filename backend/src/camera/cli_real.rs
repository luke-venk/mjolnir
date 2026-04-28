use crate::camera::AtlasATP124SResolution;
use clap::Parser;
use std::path::PathBuf;

/// Optional arguments for real backend targets.
#[derive(Parser, Debug, Clone)]
#[command(name = "real_backend_args")]
#[command(about = "Runs the real backend against recorded footage replay.")]
pub struct RealBackendArgs {
    /// Replay a folder produced by `//backend:record`.
    #[arg(long = "feed-footage-dir")]
    pub feed_footage_dir: Option<PathBuf>,

    /// Network interface for the PTP action-command master when reading real cameras.
    #[arg(long)]
    pub interface: Option<String>,

    /// Resolution of the live capture path.
    #[arg(long, value_enum, default_value_t = AtlasATP124SResolution::Full)]
    pub resolution: AtlasATP124SResolution,

    /// Exposure time in microseconds.
    #[arg(long = "exposure-us", default_value_t = 10000.0)]
    pub exposure_time_us: f64,

    /// Desired frames per second.
    #[arg(long = "frame-rate-hz", default_value_t = 30.0)]
    pub frame_rate_hz: f64,

    /// Number of queued frame buffers.
    #[arg(long, default_value_t = 16)]
    pub num_buffers: usize,

    /// Timeout for waiting on a frame buffer, in milliseconds.
    #[arg(long, default_value_t = 5000)]
    pub timeout_ms: u64,

    /// Whether to enable Precision Time Protocol while reading real cameras.
    #[arg(long, default_value_t = true)]
    pub enable_ptp: bool,
}

impl RealBackendArgs {
    pub fn validate(&self) -> Result<(), String> {
        if self.feed_footage_dir.is_none() {
            return Err("This backend now requires --feed-footage-dir <SESSION_DIR>.".to_string());
        }
        Ok(())
    }
}

pub fn parse_real_backend_args() -> RealBackendArgs {
    RealBackendArgs::parse()
}
