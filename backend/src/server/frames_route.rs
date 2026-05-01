// Static-file route that serves recorded camera frames to the frontend.
//
// The recorder writes 8-bit grayscale TIFF files under
// `<feed_footage_dir>/<sub>/frame_NNNN.tiff`. Browsers don't render TIFF
// reliably, so this handler decodes the TIFF and re-encodes as PNG before
// responding.
//
// Security: the requested path is canonicalized and verified to live under
// the canonicalized `frames_dir`, so `..` segments and symlink escape
// can't reach files outside the configured root.

use crate::server::app_state::AppState;
use axum::{
    body::Body,
    extract::{Path as AxumPath, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};
use std::io::{BufReader, Cursor};
use std::path::{Path, PathBuf};

/// Resolves `requested` (a path relative to `frames_dir`) into an absolute,
/// canonical path that is guaranteed to live under `frames_dir`. Returns
/// `None` if the path traverses outside the root, doesn't exist, or fails
/// to canonicalize for any other reason.
pub fn resolve_safe_path(frames_dir: &Path, requested: &str) -> Option<PathBuf> {
    if requested.is_empty() {
        return None;
    }

    let canonical_root = frames_dir.canonicalize().ok()?;
    let joined = canonical_root.join(requested);
    let canonical_target = joined.canonicalize().ok()?;

    if canonical_target.starts_with(&canonical_root) {
        Some(canonical_target)
    } else {
        None
    }
}

/// Decodes the TIFF at `path` and re-encodes its pixel data as PNG bytes.
fn tiff_to_png(path: &Path) -> Result<Vec<u8>, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("open {}: {e}", path.display()))?;
    let mut decoder = tiff::decoder::Decoder::new(BufReader::new(file))
        .map_err(|e| format!("tiff decoder for {}: {e}", path.display()))?;
    let (width, height) = decoder
        .dimensions()
        .map_err(|e| format!("tiff dimensions for {}: {e}", path.display()))?;
    let pixels = match decoder
        .read_image()
        .map_err(|e| format!("tiff decode for {}: {e}", path.display()))?
    {
        tiff::decoder::DecodingResult::U8(buf) => buf,
        _ => return Err(format!("expected 8-bit grayscale TIFF in {}", path.display())),
    };

    let buffer: image::ImageBuffer<image::Luma<u8>, Vec<u8>> =
        image::ImageBuffer::from_raw(width, height, pixels)
            .ok_or_else(|| format!("pixel buffer size mismatch for {}", path.display()))?;

    let mut png_bytes = Cursor::new(Vec::new());
    image::DynamicImage::ImageLuma8(buffer)
        .write_to(&mut png_bytes, image::ImageFormat::Png)
        .map_err(|e| format!("png encode for {}: {e}", path.display()))?;
    Ok(png_bytes.into_inner())
}

pub async fn get_frame(
    State(state): State<AppState>,
    AxumPath(requested_path): AxumPath<String>,
) -> Response {
    let Some(frames_dir) = state.frames_dir.as_ref() else {
        return (StatusCode::NOT_FOUND, "frames not available in this mode").into_response();
    };

    let Some(safe_path) = resolve_safe_path(frames_dir, &requested_path) else {
        return (StatusCode::NOT_FOUND, "frame not found").into_response();
    };

    match tokio::task::spawn_blocking(move || tiff_to_png(&safe_path)).await {
        Ok(Ok(bytes)) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "image/png")
            .body(Body::from(bytes))
            .expect("frame response builds"),
        Ok(Err(message)) => {
            eprintln!("frames_route: {message}");
            (StatusCode::INTERNAL_SERVER_ERROR, "failed to convert frame").into_response()
        }
        Err(join_err) => {
            eprintln!("frames_route: blocking task panicked: {join_err}");
            (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs::{self, File};
    use std::io::BufWriter;
    use std::time::{SystemTime, UNIX_EPOCH};
    use tiff::encoder::{colortype, TiffEncoder};

    fn make_temp_dir(suffix: &str) -> PathBuf {
        let dir = env::temp_dir().join(format!(
            "mjolnir_frames_{suffix}_{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time after epoch")
                .as_nanos()
        ));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn write_test_tiff(path: &Path, width: u32, height: u32, fill: u8) {
        let file = File::create(path).expect("create tiff");
        let mut writer = BufWriter::new(file);
        let mut encoder = TiffEncoder::new(&mut writer).expect("tiff encoder");
        let pixels = vec![fill; (width * height) as usize];
        encoder
            .write_image::<colortype::Gray8>(width, height, &pixels)
            .expect("write tiff");
    }

    #[test]
    fn resolve_safe_path_accepts_file_under_root() {
        let root = make_temp_dir("safe_under_root");
        let file = root.join("frame.tiff");
        fs::write(&file, b"hi").unwrap();

        let resolved = resolve_safe_path(&root, "frame.tiff").expect("should resolve");
        assert_eq!(resolved, file.canonicalize().unwrap());

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolve_safe_path_rejects_parent_traversal() {
        let root = make_temp_dir("parent_traversal");
        let outside = root.parent().unwrap().join("escape.txt");
        fs::write(&outside, b"secret").unwrap();

        assert!(resolve_safe_path(&root, "../escape.txt").is_none());

        let _ = fs::remove_file(outside);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolve_safe_path_rejects_absolute_path() {
        let root = make_temp_dir("absolute_rejected");
        // /etc/hosts exists on every unix and is outside the root.
        assert!(resolve_safe_path(&root, "/etc/hosts").is_none());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolve_safe_path_rejects_missing_file() {
        let root = make_temp_dir("missing");
        assert!(resolve_safe_path(&root, "does_not_exist.tiff").is_none());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolve_safe_path_rejects_empty_string() {
        let root = make_temp_dir("empty");
        assert!(resolve_safe_path(&root, "").is_none());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolve_safe_path_accepts_nested_subdirectory() {
        let root = make_temp_dir("nested");
        let sub = root.join("left_cam");
        fs::create_dir_all(&sub).unwrap();
        let file = sub.join("frame_0001.tiff");
        fs::write(&file, b"hi").unwrap();

        let resolved =
            resolve_safe_path(&root, "left_cam/frame_0001.tiff").expect("should resolve");
        assert_eq!(resolved, file.canonicalize().unwrap());

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn tiff_to_png_round_trips_dimensions_and_grayscale_value() {
        let root = make_temp_dir("png_roundtrip");
        let tiff_path = root.join("frame.tiff");
        write_test_tiff(&tiff_path, 8, 4, 0x42);

        let png_bytes = tiff_to_png(&tiff_path).expect("should encode PNG");

        // Decode the produced PNG and verify it carries the same pixels.
        let decoded = image::load_from_memory_with_format(&png_bytes, image::ImageFormat::Png)
            .expect("decode produced PNG");
        let gray = decoded.to_luma8();
        assert_eq!(gray.dimensions(), (8, 4));
        for pixel in gray.pixels() {
            assert_eq!(pixel.0[0], 0x42);
        }

        let _ = fs::remove_dir_all(root);
    }
}
