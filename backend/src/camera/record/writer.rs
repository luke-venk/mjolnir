/// Code that handles writing our captured frames from RAM to disk (SSD) in
/// a performant manner so frames aren't dropped.
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde_json::to_string_pretty;

/// Helper function to ensure output directory exists.
pub fn ensure_dir(path: &PathBuf) {
    fs::create_dir_all(path)
        .unwrap_or_else(|e| panic!("Failed to create directory {}: {e}", path.display()));
}

/// Helper function to create output directory.
pub fn string_to_pathbuf(path: &String) -> PathBuf {
    PathBuf::from(path)
}

/// Helper function to format timestamp string.
fn timestamp_string() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before UNIX_EPOCH");
    format!("{}_{}", now.as_secs(), now.subsec_nanos())
}

/// Helper function to ensure string values are safe paths.
pub fn sanitize_path_name(value: &str) -> String {
    value
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Writes the captured frame and metadata to disk.
pub fn write_frame_files(
    output_dir: &PathBuf,
    camera_id: &str,
    frame_index: usize,
    data: &[u8],
    metadata: &FrameMetadata,
) {
    let basename = format!(
        "{}_frame_{:06}_{}",
        sanitize_path_name(camera_id),
        frame_index,
        timestamp_string()
    );

    let raw_path = output_dir.join(format!("{basename}.raw"));
    let json_path = output_dir.join(format!("{basename}.json"));

    let mut raw_file = File::create(&raw_path)
        .unwrap_or_else(|e| panic!("failed to create {}: {e}", raw_path.display()));
    raw_file
        .write_all(data)
        .unwrap_or_else(|e| panic!("failed to write {}: {e}", raw_path.display()));

    let json = to_string_pretty(metadata).expect("failed to serialize frame metadata");
    let mut json_file = File::create(&json_path)
        .unwrap_or_else(|e| panic!("failed to create {}: {e}", json_path.display()));
    json_file
        .write_all(json.as_bytes())
        .unwrap_or_else(|e| panic!("failed to write {}: {e}", json_path.display()));
}

/// Metadata for each frame to be recorded in addition to raw files.
#[derive(Debug, Serialize)]
pub struct FrameMetadata {
    pub camera_id: String,
    pub frame_index: usize,
    pub width: i32,
    pub height: i32,
    pub payload_bytes: usize,
    pub system_timestamp_ns: u64,
    pub buffer_timestamp_ns: u64,
    pub frame_id: u64,
    pub exposure_time_us: f64,
    pub frame_rate_hz: f64,
}
