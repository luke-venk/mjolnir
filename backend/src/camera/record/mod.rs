pub mod cli;
pub mod config;
pub mod record_from_one_camera;
pub mod writer;

pub use config::{CameraIngestConfig, Resolution};
pub use record_from_one_camera::record_from_one_camera;
