use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use aravis::{BufferStatus, CameraExt, StreamExt};

use crate::camera::CameraIngestConfig;
use crate::camera::aravis_utils::{
    configure_camera, copy_buffer_bytes, create_camera, create_stream_and_allocate_buffers,
};

use super::h265_stream::{H265CameraEncoder, recover_h265_to_png, sanitize};

pub fn record_h265_from_one_camera(
    config: &CameraIngestConfig,
    output_base_dir: &Path,
    recover_base_dir: Option<&Path>,
    max_frames: Option<usize>,
    max_duration: Option<f64>,
) -> Result<()> {
    config.validate().map_err(anyhow::Error::msg)?;

    let output_camera_dir = output_base_dir.join(sanitize(&config.camera_id));
    fs::create_dir_all(&output_camera_dir)
        .with_context(|| format!("create directory {}", output_camera_dir.display()))?;

    println!("-------------------------");
    println!("Opening camera {}", config.camera_id);
    println!("-------------------------");

    let camera = create_camera(&config.camera_id).map_err(anyhow::Error::msg)?;
    configure_camera(&camera, config);
    let stream = create_stream_and_allocate_buffers(&camera, config.num_buffers);

    let (_, _, width, height) = camera
        .region()
        .context("read camera region after configuration")?;
    let payload = camera
        .payload()
        .context("read camera payload after configuration")?;
    println!(
        "Configured camera {}: width={} height={} payload={} exposure_us={} frame_rate_hz={}",
        config.camera_id, width, height, payload, config.exposure_time_us, config.frame_rate_hz
    );

    let width_u32 = u32::try_from(width).context("camera width does not fit into u32")?;
    let height_u32 = u32::try_from(height).context("camera height does not fit into u32")?;
    // Create one long-lived lossless H.265 encoder for this camera instead of writing raw frames.
    let mut encoder = H265CameraEncoder::new(
        &output_camera_dir,
        &config.camera_id,
        width_u32,
        height_u32,
        config.frame_rate_hz,
    )?;

    camera
        .start_acquisition()
        .context("start camera acquisition")?;

    let start_time = Instant::now();
    let mut frames_written = 0u64;
    let mut first_buffer_arrived = false;

    let recording_result: Result<()> = (|| {
        loop {
            if let Some(limit) = max_frames {
                if frames_written >= limit as u64 {
                    break;
                }
            }

            if let Some(seconds) = max_duration {
                if start_time.elapsed() >= Duration::from_secs_f64(seconds) {
                    break;
                }
            }

            let buffer = match stream.timeout_pop_buffer(config.timeout_ms.saturating_mul(1000)) {
                Some(buffer) => {
                    if !first_buffer_arrived {
                        first_buffer_arrived = true;
                    }
                    buffer
                }
                None => {
                    if first_buffer_arrived {
                        eprintln!(
                            "Timed out waiting for frame buffer to be delivered from camera {}.",
                            config.camera_id
                        );
                    }
                    continue;
                }
            };

            match buffer.status() {
                BufferStatus::Success => {
                    let data = copy_buffer_bytes(&buffer);

                    if data.is_empty() {
                        eprintln!("Empty buffer from camera {}.", config.camera_id);
                        stream.push_buffer(buffer);
                        continue;
                    }

                    encoder.push_frame(&data)?;
                    frames_written += 1;

                    if frames_written % 10 == 0 {
                        println!(
                            "Camera {}: encoded {} frame(s) to lossless H.265",
                            config.camera_id, frames_written
                        );
                    }
                }
                status => {
                    eprintln!(
                        "Camera {} returned non-success buffer status: {:?}",
                        config.camera_id, status
                    );
                }
            }

            stream.push_buffer(buffer);
        }

        Ok(())
    })();

    let _ = camera.stop_acquisition();
    recording_result?;

    let summary = encoder.finish()?;
    println!(
        "Finished camera {}. Wrote {} frame(s) to {}",
        config.camera_id,
        summary.frames_written,
        summary.h265_path.display()
    );

    if let Some(recover_base_dir) = recover_base_dir {
        let png_dir = recover_base_dir.join(sanitize(&config.camera_id));
        // Optional same-command recovery: decode the recorded H.265 stream back into PNG frames.
        let recovery = recover_h265_to_png(&summary.h265_path, &png_dir)?;
        println!(
            "Recovered {} PNG frame(s) for camera {} into {}",
            recovery.frames_recovered,
            config.camera_id,
            recovery.output_dir.display()
        );
    }

    Ok(())
}
