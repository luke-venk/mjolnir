// Compresses per-frame MONO8 recordings with zstd and recovers them to PNG.
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Cursor};
use std::path::Path;

use anyhow::{Context, Result, bail};
use png::{BitDepth, ColorType, Encoder as PngEncoder};

use crate::camera::record::writer::FrameMetadata;

pub const COMPRESSED_FRAME_EXTENSION: &str = "zst";
const ZSTD_LEVEL: i32 = 1;

// Compresses one raw MONO8 frame with a fast zstd setting.
pub fn compress_mono8_frame(raw_bytes: &[u8]) -> Result<Vec<u8>> {
    zstd::bulk::compress(raw_bytes, ZSTD_LEVEL).context("compress MONO8 frame with zstd")
}

// Decompresses one zstd-compressed MONO8 frame and checks its expected size.
fn decompress_mono8_frame(compressed_bytes: &[u8], expected_len: usize) -> Result<Vec<u8>> {
    let raw_bytes =
        zstd::stream::decode_all(Cursor::new(compressed_bytes)).context("decompress zstd frame")?;

    if raw_bytes.len() != expected_len {
        bail!(
            "decompressed frame size mismatch: got {} bytes, expected {}",
            raw_bytes.len(),
            expected_len
        );
    }

    Ok(raw_bytes)
}

// Writes one MONO8 frame as a grayscale PNG file.
fn write_mono8_png(output_path: &Path, width: u32, height: u32, raw_bytes: &[u8]) -> Result<()> {
    let file =
        File::create(output_path).with_context(|| format!("create {}", output_path.display()))?;
    let writer = BufWriter::new(file);
    let mut encoder = PngEncoder::new(writer, width, height);
    encoder.set_color(ColorType::Grayscale);
    encoder.set_depth(BitDepth::Eight);

    let mut png_writer = encoder
        .write_header()
        .with_context(|| format!("write PNG header {}", output_path.display()))?;
    png_writer
        .write_image_data(raw_bytes)
        .with_context(|| format!("write PNG bytes {}", output_path.display()))?;

    Ok(())
}

// Recovers a directory of compressed MONO8 frames into PNG files.
pub fn recover_compressed_dir_to_pngs(input_dir: &Path, output_dir: &Path) -> Result<u64> {
    fs::create_dir_all(output_dir)
        .with_context(|| format!("create output dir {}", output_dir.display()))?;

    let mut frame_paths = Vec::new();
    for entry in
        fs::read_dir(input_dir).with_context(|| format!("read input dir {}", input_dir.display()))?
    {
        let entry = entry.with_context(|| format!("read input dir {}", input_dir.display()))?;
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == COMPRESSED_FRAME_EXTENSION) {
            frame_paths.push(path);
        }
    }
    frame_paths.sort();

    if frame_paths.is_empty() {
        bail!(
            "input directory {} does not contain any .{} frame files",
            input_dir.display(),
            COMPRESSED_FRAME_EXTENSION
        );
    }

    for frame_path in &frame_paths {
        let metadata_path = frame_path.with_extension("json");
        let metadata_file = File::open(&metadata_path)
            .with_context(|| format!("open metadata {}", metadata_path.display()))?;
        let metadata_reader = BufReader::new(metadata_file);
        let metadata: FrameMetadata = serde_json::from_reader(metadata_reader)
            .with_context(|| format!("read metadata {}", metadata_path.display()))?;

        let compressed_bytes = fs::read(frame_path)
            .with_context(|| format!("read compressed frame {}", frame_path.display()))?;
        let raw_bytes = decompress_mono8_frame(&compressed_bytes, metadata.payload_bytes)?;
        let png_path = output_dir.join(format!("frame_{:06}.png", metadata.frame_index));

        let width = u32::try_from(metadata.width).context("metadata width does not fit into u32")?;
        let height =
            u32::try_from(metadata.height).context("metadata height does not fit into u32")?;
        write_mono8_png(&png_path, width, height, &raw_bytes)?;
    }

    Ok(frame_paths.len() as u64)
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{compress_mono8_frame, recover_compressed_dir_to_pngs};
    use crate::camera::record::writer::{FrameMetadata, write_frame_files};

    fn temp_test_dir(name: &str) -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock before unix epoch")
            .as_nanos();
        env::temp_dir().join(format!(
            "mjolnir_compressed_frame_tests_{name}_{}_{}",
            std::process::id(),
            unique
        ))
    }

    /// Confirms that one compressed frame can be recovered to one PNG.
    #[test]
    fn recover_compressed_dir_to_pngs_recovers_one_frame() {
        let input_dir = temp_test_dir("input");
        let output_dir = temp_test_dir("output");
        fs::create_dir_all(&input_dir).expect("create input dir");

        let raw_bytes = vec![128u8; 16];
        let compressed_bytes = compress_mono8_frame(&raw_bytes).expect("compress frame");
        let metadata = FrameMetadata {
            camera_id: "camera".to_string(),
            frame_index: 0,
            width: 4,
            height: 4,
            payload_bytes: raw_bytes.len(),
            system_timestamp_ns: 0,
            buffer_timestamp_ns: 0,
            frame_id: 0,
            exposure_time_us: 1000.0,
            frame_rate_hz: 30.0,
        };

        write_frame_files(
            &input_dir,
            "camera",
            0,
            &compressed_bytes,
            &metadata,
            "zst",
        );

        let recovered = recover_compressed_dir_to_pngs(&input_dir, &output_dir)
            .expect("recover compressed frames");
        assert_eq!(recovered, 1);
        assert!(output_dir.join("frame_000000.png").exists());

        fs::remove_dir_all(&input_dir).expect("cleanup input dir");
        fs::remove_dir_all(&output_dir).expect("cleanup output dir");
    }
}
