// Code that handles writing our captured frames from RAM to disk (SSD) in
// a performant manner so frames aren't dropped.
use serde::{Deserialize, Serialize};
use serde_json::to_string_pretty;
use std::fs::{self, File, create_dir_all};
use std::io::BufReader;
use std::io::BufWriter;
use std::io::Write;
use std::path::{Path, PathBuf};
use tiff::decoder::{Decoder, DecodingResult};
use tiff::encoder::{TiffEncoder, colortype};

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
    pub width: u32,
    pub height: u32,
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
    let file_name = format!("frame_{:04}", frame_index,);
    let raw_path = output_camera_dir.join(format!("{file_name}.tiff"));
    let json_path = output_camera_dir.join(format!("{file_name}.json"));

    let raw_file = File::create(&raw_path)
        .unwrap_or_else(|e| panic!("failed to create {}: {e}", raw_path.display()));
    let mut writer = BufWriter::with_capacity(1024 * 1024, raw_file); // 1MB buffer
    let mut encoder = TiffEncoder::new(&mut writer).expect("Failed to create tiff encoder");
    encoder
        .write_image::<colortype::Gray8>(metadata.width as u32, metadata.height as u32, data)
        .expect(&format!("failed to write {}", raw_path.display()));

    let json = to_string_pretty(metadata).expect("failed to serialize frame metadata");
    let mut json_file = File::create(&json_path)
        .unwrap_or_else(|e| panic!("failed to create {}: {e}", json_path.display()));
    json_file
        .write_all(json.as_bytes())
        .unwrap_or_else(|e| panic!("failed to write {}: {e}", json_path.display()));
}

/// Reads one recorded frame payload + its metadata back from disk.
///
/// `json_path` must point to a frame metadata file. The matching payload is
/// looked up next to it by trying the extensions in
/// `RECORDED_FRAME_PAYLOAD_EXTENSIONS` in order (`.tiff` then `.raw`).
pub fn read_recorded_frame(json_path: &Path) -> Frame {
    let metadata_json = fs::read_to_string(json_path)
        .unwrap_or_else(|err| panic!("Failed to read {}: {err}", json_path.display()));
    let metadata: Metadata = serde_json::from_str(&metadata_json)
        .unwrap_or_else(|err| panic!("Failed to parse {}: {err}", json_path.display()));
    let bytes = read_recorded_frame_payload_bytes(json_path);
    let output_camera_dir = json_path
        .parent()
        .unwrap_or_else(|| {
            panic!(
                "Frame metadata {} has no parent directory",
                json_path.display()
            )
        })
        .to_path_buf();

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
            if payload_path.extension().is_some_and(|ext| ext == "tiff") {
                return read_tiff_payload_bytes(payload_path);
            }
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

fn read_tiff_payload_bytes(tiff_path: &Path) -> Vec<u8> {
    let file = File::open(tiff_path)
        .unwrap_or_else(|err| panic!("Failed to open {}: {err}", tiff_path.display()));
    let mut decoder = Decoder::new(BufReader::new(file)).unwrap_or_else(|err| {
        panic!(
            "Failed to create TIFF decoder for {}: {err}",
            tiff_path.display()
        )
    });

    match decoder
        .read_image()
        .unwrap_or_else(|err| panic!("Failed to decode {}: {err}", tiff_path.display()))
    {
        DecodingResult::U8(bytes) => bytes,
        _ => panic!(
            "Expected an 8-bit grayscale TIFF payload in {}.",
            tiff_path.display()
        ),
    }
}
