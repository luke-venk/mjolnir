/// Code that handles writing our captured frames from RAM to disk in
/// so frames aren't dropped.
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde_json::to_writer;

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

/// Paths written for one saved frame.
pub struct WrittenFramePaths {
    pub frame_path: PathBuf,
    pub metadata_path: PathBuf,
}

/// Writes frame metadata as compact JSON.
fn write_json_file(path: &PathBuf, metadata: &FrameMetadata) {
    let file = File::create(path)
        .unwrap_or_else(|e| panic!("failed to create {}: {e}", path.display()));
    let mut writer = BufWriter::new(file);
    to_writer(&mut writer, metadata).expect("failed to serialize frame metadata");
    writer
        .flush()
        .unwrap_or_else(|e| panic!("failed to flush {}: {e}", path.display()));
}

/// Writes metadata for one frame when the compressed stream itself is being written elsewhere.
pub fn write_frame_metadata_file(
    output_dir: &PathBuf,
    frame_index: usize,
    metadata: &FrameMetadata,
) -> PathBuf {
    let json_path = output_dir.join(format!("frame_{frame_index:06}.json"));
    write_json_file(&json_path, metadata);
    json_path
}

/// Writes one frame's bytes plus metadata to disk.
pub fn write_frame_files(
    output_dir: &PathBuf,
    camera_id: &str,
    frame_index: usize,
    data: &[u8],
    metadata: &FrameMetadata,
    data_extension: &str,
) -> WrittenFramePaths {
    let basename = format!(
        "{}_frame_{:06}_{}",
        sanitize_path_name(camera_id),
        frame_index,
        timestamp_string()
    );

    let frame_path = output_dir.join(format!("{basename}.{data_extension}"));
    let json_path = output_dir.join(format!("{basename}.json"));

    let mut frame_file = File::create(&frame_path)
        .unwrap_or_else(|e| panic!("failed to create {}: {e}", frame_path.display()));
    frame_file
        .write_all(data)
        .unwrap_or_else(|e| panic!("failed to write {}: {e}", frame_path.display()));

    write_json_file(&json_path, metadata);

    WrittenFramePaths {
        frame_path,
        metadata_path: json_path,
    }
}

/// Metadata for each frame to be recorded in addition to raw files.
#[derive(Debug, Clone, Serialize)]
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
