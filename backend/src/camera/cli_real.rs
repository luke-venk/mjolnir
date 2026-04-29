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
    #[arg(long = "feed-footage-dir", required = true)]
    pub feed_footage_dir: PathBuf,
}

pub fn parse_real_backend_args() -> RealBackendArgs {
    RealBackendArgs::parse()
}
