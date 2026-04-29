use clap::Parser;

/// Command line arguments for running the real-camera CV backend pipelines.
#[derive(Parser, Debug, Clone)]
#[command(name = "real_backend_args")]
#[command(about = "Runs the real-camera CV backend pipelines with explicit left/right camera names.")]
pub struct RealBackendArgs {
    /// Camera identifier for the left-field camera pipeline.
    #[arg(long = "left-camera", required = true)]
    pub left_camera_id: String,

    /// Camera identifier for the right-field camera pipeline.
    #[arg(long = "right-camera", required = true)]
    pub right_camera_id: String,
}

pub fn parse_real_backend_args() -> RealBackendArgs {
    RealBackendArgs::parse()
}
