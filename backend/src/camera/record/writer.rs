// Code that handles writing our captured frames from RAM to disk (SSD) in
// a performant manner so frames aren't dropped.
use serde::{Deserialize, Serialize};
use serde_json::to_string_pretty;
use std::fs::{self, File, create_dir_all};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use tiff::encoder::{TiffEncoder, colortype};

pub const SESSION_MANIFEST_FILE_NAME: &str = "recording_session.json";
const RECORDED_FRAME_PAYLOAD_EXTENSIONS: [&str; 2] = ["tiff", "raw"];

/// Helper function to ensure output directory exists.
pub fn ensure_dir(path: &PathBuf) {
    create_dir_all(path)
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
    let file_name = format!("frame_{:04}", frame_index);
    let tiff_path = output_camera_dir.join(format!("{file_name}.tiff"));
    let json_path = output_camera_dir.join(format!("{file_name}.json"));

    let tiff_file = File::create(&tiff_path)
        .unwrap_or_else(|e| panic!("failed to create {}: {e}", tiff_path.display()));
    let mut writer = BufWriter::with_capacity(1024 * 1024, tiff_file);
    let mut encoder = TiffEncoder::new(&mut writer).expect("Failed to create tiff encoder");
    encoder
        .write_image::<colortype::Gray8>(metadata.width as u32, metadata.height as u32, data)
        .unwrap_or_else(|_| panic!("failed to write {}", tiff_path.display()));

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
    let bytes = read_recorded_frame_payload_bytes(json_path);
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

fn read_recorded_frame_payload_bytes(json_path: &Path) -> Vec<u8> {
    let payload_paths = RECORDED_FRAME_PAYLOAD_EXTENSIONS
        .iter()
        .map(|extension| json_path.with_extension(extension))
        .collect::<Vec<_>>();

    for payload_path in &payload_paths {
        if payload_path.exists() {
            return fs::read(payload_path)
                .unwrap_or_else(|err| panic!("Failed to read {}: {err}", payload_path.display()));
        }
    }

    let attempted_paths = payload_paths
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");
    panic!(
        "Failed to find recorded frame payload for {}. Tried: {}",
        json_path.display(),
        attempted_paths
    );
}

#[cfg(test)]
mod tests {
    use std::{env, fs};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{Metadata, read_recorded_frame};

    #[test]
    fn read_recorded_frame_supports_tiff_payloads() {
        let temp_dir = env::temp_dir().join(format!(
            "mjolnir_recorded_frame_tiff_test_{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time should be after epoch")
                .as_nanos()
        ));
        fs::create_dir_all(&temp_dir).expect("create temp dir");
        let metadata = Metadata {
            camera_id: "camera-left".to_string(),
            frame_index: 0,
            width: 2,
            height: 1,
            payload_bytes: 2,
            system_timestamp_ns: 123,
            buffer_timestamp_ns: 456,
            frame_id: 789,
        };
        let json_path = temp_dir.join("frame_0000.json");
        fs::write(
            &json_path,
            serde_json::to_vec_pretty(&metadata).expect("serialize metadata"),
        )
        .expect("write metadata");
        fs::write(temp_dir.join("frame_0000.tiff"), [7u8, 8u8]).expect("write tiff payload");

        let frame = read_recorded_frame(&json_path);

        assert_eq!(frame.bytes, vec![7, 8]);
        assert_eq!(frame.metadata.frame_id, 789);

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn read_recorded_frame_prefers_tiff_over_raw_when_both_exist() {
        let temp_dir = env::temp_dir().join(format!(
            "mjolnir_recorded_frame_tiff_priority_test_{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time should be after epoch")
                .as_nanos()
        ));
        fs::create_dir_all(&temp_dir).expect("create temp dir");
        let metadata = Metadata {
            camera_id: "camera-left".to_string(),
            frame_index: 0,
            width: 2,
            height: 1,
            payload_bytes: 2,
            system_timestamp_ns: 123,
            buffer_timestamp_ns: 456,
            frame_id: 789,
        };
        let json_path = temp_dir.join("frame_0000.json");
        fs::write(
            &json_path,
            serde_json::to_vec_pretty(&metadata).expect("serialize metadata"),
        )
        .expect("write metadata");
        fs::write(temp_dir.join("frame_0000.raw"), [1u8, 2u8]).expect("write raw payload");
        fs::write(temp_dir.join("frame_0000.tiff"), [7u8, 8u8]).expect("write tiff payload");

        let frame = read_recorded_frame(&json_path);

        assert_eq!(frame.bytes, vec![7, 8]);

        let _ = fs::remove_dir_all(temp_dir);
    }
}
