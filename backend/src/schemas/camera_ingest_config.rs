use crate::schemas::CameraId;
use crate::tools::env::{read_env_bool, read_env_number, read_env_string};

const DEFAULT_EXPOSURE_US: f64 = 10_000.0;
const DEFAULT_FRAME_RATE_HZ: f64 = 30.0;
const DEFAULT_APERTURE: f64 = 0.0;
const DEFAULT_NUM_BUFFERS: usize = 16;
const DEFAULT_TIMEOUT_MS: u64 = 200;
const DEFAULT_ENABLE_PTP: bool = false;
const DEFAULT_USE_FAKE_INTERFACE: bool = false;

#[derive(Debug, Clone)]
pub struct CameraIngestConfig {
    pub device_id: Option<String>,
    pub exposure_time_us: f64,
    pub frame_rate_hz: f64,
    pub aperture: f64,
    pub enable_ptp: bool,
    pub use_fake_interface: bool,
    pub num_buffers: usize,
    pub timeout_ms: u64,
}

impl CameraIngestConfig {
    pub fn new(camera_id: CameraId) -> Self {
        let prefix = match camera_id {
            CameraId::FieldLeft => "ARAVIS_LEFT",
            CameraId::FieldRight => "ARAVIS_RIGHT",
        };

        Self {
            device_id: read_env_string(&format!("{prefix}_CAMERA_ID")),
            exposure_time_us: read_env_number(&format!("{prefix}_EXPOSURE_US"))
                .unwrap_or(DEFAULT_EXPOSURE_US),
            frame_rate_hz: read_env_number(&format!("{prefix}_FRAME_RATE_HZ"))
                .unwrap_or(DEFAULT_FRAME_RATE_HZ),
            aperture: read_env_number(&format!("{prefix}_APERTURE"))
                .unwrap_or(DEFAULT_APERTURE),
            enable_ptp: read_env_bool(
                &format!("{prefix}_ENABLE_PTP"),
                DEFAULT_ENABLE_PTP,
            ),
            use_fake_interface: read_env_bool(
                &format!("{prefix}_USE_FAKE_INTERFACE"),
                DEFAULT_USE_FAKE_INTERFACE,
            ),
            num_buffers: read_env_number(&format!("{prefix}_NUM_BUFFERS"))
                .unwrap_or(DEFAULT_NUM_BUFFERS),
            timeout_ms: read_env_number(&format!("{prefix}_TIMEOUT_MS"))
                .unwrap_or(DEFAULT_TIMEOUT_MS),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn loads_left_camera_config_from_left_prefix() {
        unsafe {
            env::set_var("ARAVIS_LEFT_CAMERA_ID", "left-cam");
            env::set_var("ARAVIS_LEFT_EXPOSURE_US", "12000");
            env::set_var("ARAVIS_LEFT_FRAME_RATE_HZ", "25");
            env::set_var("ARAVIS_LEFT_APERTURE", "1.8");
            env::set_var("ARAVIS_LEFT_ENABLE_PTP", "true");
            env::set_var("ARAVIS_LEFT_USE_FAKE_INTERFACE", "false");
            env::set_var("ARAVIS_LEFT_NUM_BUFFERS", "8");
            env::set_var("ARAVIS_LEFT_TIMEOUT_MS", "500");
        }

        let config = CameraIngestConfig::new(CameraId::FieldLeft);

        assert_eq!(config.device_id, Some("left-cam".to_string()));
        assert_eq!(config.exposure_time_us, 12000.0);
        assert_eq!(config.frame_rate_hz, 25.0);
        assert_eq!(config.aperture, 1.8);
        assert!(config.enable_ptp);
        assert!(!config.use_fake_interface);
        assert_eq!(config.num_buffers, 8);
        assert_eq!(config.timeout_ms, 500);

        unsafe {
            env::remove_var("ARAVIS_LEFT_CAMERA_ID");
            env::remove_var("ARAVIS_LEFT_EXPOSURE_US");
            env::remove_var("ARAVIS_LEFT_FRAME_RATE_HZ");
            env::remove_var("ARAVIS_LEFT_APERTURE");
            env::remove_var("ARAVIS_LEFT_ENABLE_PTP");
            env::remove_var("ARAVIS_LEFT_USE_FAKE_INTERFACE");
            env::remove_var("ARAVIS_LEFT_NUM_BUFFERS");
            env::remove_var("ARAVIS_LEFT_TIMEOUT_MS");
        }
    }
}