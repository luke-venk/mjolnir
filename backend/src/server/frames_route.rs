// Serves the latest impact frame for a given camera as PNG.
//
// The pipeline publishes raw grayscale bytes + dimensions into
// `AppState::{left,right}_impact_frame` when a throw completes. This
// handler reads that slot, PNG-encodes the bytes on demand, and returns
// `image/png`. If the slot is empty (no throw yet, or no impact frame
// produced) it returns 404.
//
// The route lives at `GET /api/frames/{camera}` where `{camera}` is
// "left" or "right". Anything else is a 404.

use crate::server::app_state::{AppState, ImpactFrame};
use axum::{
    body::Body,
    extract::{Path as AxumPath, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};
use std::io::Cursor;

/// Encodes a raw 8-bit grayscale frame as a PNG.
fn impact_frame_to_png(frame: &ImpactFrame) -> Result<Vec<u8>, String> {
    let (bytes, (width, height)) = frame;
    let buffer: image::ImageBuffer<image::Luma<u8>, Vec<u8>> =
        image::ImageBuffer::from_raw(*width, *height, bytes.clone())
            .ok_or_else(|| format!("pixel buffer size mismatch ({width}x{height})"))?;
    let mut png_bytes = Cursor::new(Vec::new());
    image::DynamicImage::ImageLuma8(buffer)
        .write_to(&mut png_bytes, image::ImageFormat::Png)
        .map_err(|e| format!("png encode: {e}"))?;
    Ok(png_bytes.into_inner())
}

pub async fn get_frame(
    State(state): State<AppState>,
    AxumPath(camera): AxumPath<String>,
) -> Response {
    let slot = match camera.as_str() {
        "left" => &state.left_impact_frame,
        "right" => &state.right_impact_frame,
        _ => return (StatusCode::NOT_FOUND, "unknown camera").into_response(),
    };

    let frame = match slot.read().await.clone() {
        Some(f) => f,
        None => return (StatusCode::NOT_FOUND, "no impact frame yet").into_response(),
    };

    match impact_frame_to_png(&frame) {
        Ok(bytes) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "image/png")
            .body(Body::from(bytes))
            .expect("frame response builds"),
        Err(message) => {
            eprintln!("frames_route: {message}");
            (StatusCode::INTERNAL_SERVER_ERROR, "failed to encode frame").into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn impact_frame_to_png_round_trips_dimensions_and_grayscale_value() {
        let bytes = vec![0x42u8; 8 * 4];
        let frame: ImpactFrame = (bytes, (8, 4));

        let png_bytes = impact_frame_to_png(&frame).expect("should encode PNG");

        let decoded = image::load_from_memory_with_format(&png_bytes, image::ImageFormat::Png)
            .expect("decode produced PNG");
        let gray = decoded.to_luma8();
        assert_eq!(gray.dimensions(), (8, 4));
        for pixel in gray.pixels() {
            assert_eq!(pixel.0[0], 0x42);
        }
    }

    #[test]
    fn impact_frame_to_png_rejects_size_mismatch() {
        // Claim 4x4 (16 pixels) but provide only 10 bytes.
        let frame: ImpactFrame = (vec![0u8; 10], (4, 4));
        assert!(impact_frame_to_png(&frame).is_err());
    }
}
