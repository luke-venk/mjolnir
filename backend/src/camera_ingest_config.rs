use std::env;
use std::sync::OnceLock;

const DEFAULT_NUM_BUFFERS: usize = 16;
const DEFAULT_TIMEOUT_MS: u64 = 200;
const DEFAULT_ENABLE_PTP: bool = false;
const DEFAULT_USE_FAKE_INTERFACE: bool = false;

#[derive(Debug, Clone)]
pub struct CameraIngestConfig {
    pub device_id: Option<String>,
    pub exposure_time_us: Option<f64>,
    pub frame_rate_hz: Option<f64>,
    pub aperture: Option<f64>,
    pub enable_ptp: bool,
    pub use_fake_interface: bool,
    pub num_buffers: usize,
    pub timeout_ms: u64,
}

static CONFIG: OnceLock<CameraIngestConfig> = OnceLock::new();

impl CameraIngestConfig {
    pub fn load() -> &'static Self {
        CONFIG.get_or_init(|| Self::from_env())
    }

    fn from_env() -> Self {
        Self {
            device_id: read_string_env("ARAVIS_CAMERA_ID"),
            exposure_time_us: read_f64_env("ARAVIS_EXPOSURE_US"),
            frame_rate_hz: read_f64_env("ARAVIS_FRAME_RATE_HZ"),
            aperture: read_f64_env("ARAVIS_APERTURE"),
            enable_ptp: read_bool_env("ARAVIS_ENABLE_PTP", DEFAULT_ENABLE_PTP),
            use_fake_interface: read_bool_env(
                "ARAVIS_USE_FAKE_INTERFACE",
                DEFAULT_USE_FAKE_INTERFACE,
            ),
            num_buffers: read_usize_env("ARAVIS_NUM_BUFFERS", DEFAULT_NUM_BUFFERS),
            timeout_ms: read_u64_env("ARAVIS_TIMEOUT_MS", DEFAULT_TIMEOUT_MS),
        }
    }
}

fn read_string_env(key: &str) -> Option<String> {
    env::var(key).ok().filter(|v| !v.trim().is_empty())
}

fn read_f64_env(key: &str) -> Option<f64> {
    env::var(key).ok()?.trim().parse::<f64>().ok()
}

fn read_bool_env(key: &str, default: bool) -> bool {
    match env::var(key) {
        Ok(v) => match v.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => true,
            "0" | "false" | "no" | "off" => false,
            _ => default,
        },
        Err(_) => default,
    }
}

fn read_usize_env(key: &str, default: usize) -> usize {
    env::var(key)
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .unwrap_or(default)
}

fn read_u64_env(key: &str, default: u64) -> u64 {
    env::var(key)
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .unwrap_or(default)
}