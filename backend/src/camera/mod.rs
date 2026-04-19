pub mod aravis_utils;
pub mod cli_record;
pub mod cli_stream;
pub mod config;
pub mod discovery;
pub mod record;

pub use cli_record::{RecordWithBothCamerasArgs, RecordWithOneCameraArgs};
pub use cli_stream::StreamFromCamerasArgs;
pub use config::{CameraIngestConfig, Resolution};
