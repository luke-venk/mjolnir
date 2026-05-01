use clap::Parser;
use std::path::PathBuf;

/// CLI arguments for the real backend binary when running against recorded footage.
#[derive(Parser, Debug, Clone)]
#[command(name = "real_backend_args")]
#[command(about = "Runs the real backend against a recorded footage session.")]
pub struct RealBackendArgs {
    /// Replay a session directory containing `left_cam/` and `right_cam/` subdirectories.
    #[arg(long = "feed-footage-dir", required = true)]
    pub feed_footage_dir: PathBuf,
}

pub fn parse_real_backend_args() -> RealBackendArgs {
    RealBackendArgs::parse()
}
