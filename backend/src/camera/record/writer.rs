// Code that handles writing our captured frames from RAM to disk (SSD) in
// a performant manner so frames aren't dropped.
use serde::{Deserialize, Serialize};
use serde_json::to_string_pretty;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

pub const SESSION_MANIFEST_FILE_NAME: &str = "recording_session.json";

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
#[derive(Debug, Clone, Serialize, Deserialize)]
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

/// Session-level metadata so replay can preserve left/right routing later.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionManifest {
    pub left_camera_id: String,
    pub right_camera_id: String,
}

/// Payload that the recording capture thread(s) will send over
/// crossbeam channel to writer thread.
#[derive(Debug, Clone)]
pub struct Frame {
    pub output_camera_dir: Option<PathBuf>,
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
    let file_name = format!("frame_{:04}", frame_index,);
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

/// Writes one manifest per recording session so replay mode can preserve
/// the original left/right camera assignment.
pub fn write_session_manifest(output_base_dir: &PathBuf, manifest: &SessionManifest) {
    let manifest_path = output_base_dir.join(SESSION_MANIFEST_FILE_NAME);
    let json = to_string_pretty(manifest).expect("failed to serialize session manifest");
    let mut manifest_file = File::create(&manifest_path)
        .unwrap_or_else(|e| panic!("failed to create {}: {e}", manifest_path.display()));
    manifest_file
        .write_all(json.as_bytes())
        .unwrap_or_else(|e| panic!("failed to write {}: {e}", manifest_path.display()));
}

/// Reads one manifest per recording session so replay mode can preserve
/// the original left/right camera assignment.
pub fn read_session_manifest(output_base_dir: &Path) -> Option<SessionManifest> {
    let manifest_path = output_base_dir.join(SESSION_MANIFEST_FILE_NAME);
    if !manifest_path.exists() {
        return None;
    }

    let manifest_json = fs::read_to_string(&manifest_path)
        .unwrap_or_else(|err| panic!("Failed to read {}: {err}", manifest_path.display()));
    Some(serde_json::from_str(&manifest_json).unwrap_or_else(|err| {
        panic!("Failed to parse {}: {err}", manifest_path.display())
    }))
}

/// Reads one recorded frame payload and its metadata from disk.
pub fn read_recorded_frame(json_path: &Path) -> Frame {
    let metadata_json = fs::read_to_string(json_path)
        .unwrap_or_else(|err| panic!("Failed to read {}: {err}", json_path.display()));
    let metadata: Metadata = serde_json::from_str(&metadata_json)
        .unwrap_or_else(|err| panic!("Failed to parse {}: {err}", json_path.display()));
    let raw_path = json_path.with_extension("raw");
    let bytes = fs::read(&raw_path)
        .unwrap_or_else(|err| panic!("Failed to read {}: {err}", raw_path.display()));
    let output_camera_dir = Some(
        json_path
            .parent()
            .unwrap_or_else(|| {
                panic!("Frame metadata {} has no parent directory", json_path.display())
            })
            .to_path_buf(),
    );

    Frame {
        output_camera_dir,
        frame_index: metadata.frame_index,
        bytes,
        metadata,
    }
}
