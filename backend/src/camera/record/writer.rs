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

/// Metadata for each frame to be recorded as a JSON file,
/// in addition to raw bytes for the frame.
#[derive(Debug, Clone, Serialize)]
pub struct Metadata {
    pub camera_id: String,
    pub frame_index: usize,
    pub width: i32,
    pub height: i32,
    pub payload_bytes: usize,
    pub system_timestamp_ns: u64,
    pub buffer_timestamp_ns: u64,
    pub frame_id: u64,
}

/// Payload that the recording capture thread(s) will send over 
/// crossbeam channel to writer thread.
#[derive(Debug, Clone)]
pub struct Frame {
    pub output_camera_dir: PathBuf,
    pub frame_index: usize,
    pub bytes: Vec<u8>,
    pub metadata: Metadata,
}

/// Writes the captured frame and metadata to disk.
pub fn write_to_disk(
    output_camera_dir: &PathBuf,
    frame_index: usize,
    data: &[u8],
    metadata: &Metadata,
) {
    // Determine file name based on frame index and timestamp.
    let file_name = format!(
        "frame_{:04}",
        frame_index,
    );
    let raw_path = output_camera_dir.join(format!("{file_name}.raw"));
    let json_path = output_camera_dir.join(format!("{file_name}.json"));

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
