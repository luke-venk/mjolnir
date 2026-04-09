// Pulls frames from one camera and feeds them into the lossless H.265 encoder.

use std::fs;
use std::path::Path;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use aravis::{BufferStatus, CameraExt, StreamExt};

use crate::camera::CameraIngestConfig;
use crate::camera::aravis_utils::{
    configure_camera, copy_buffer_bytes, create_camera, create_stream_and_allocate_buffers,
};
use crate::camera::record::writer::{FrameMetadata, ensure_dir, write_frame_metadata_file};

use super::encoder::H265CameraEncoder;
use super::encoder::H265SessionSummary;
use super::recovery::recover_h265_to_png;
use super::shared::sanitize;

// Builds the per-camera recording and recovery output directories.
fn prepare_camera_dirs(
    camera_id: &str,
    output_base_dir: &Path,
    recover_base_dir: Option<&Path>,
) -> Result<(std::path::PathBuf, Option<std::path::PathBuf>)> {
    let output_camera_dir = output_base_dir.join(sanitize(camera_id));
    fs::create_dir_all(&output_camera_dir)
        .with_context(|| format!("create directory {}", output_camera_dir.display()))?;

    let recover_camera_dir = recover_base_dir.map(|base| base.join(sanitize(camera_id)));
    if let Some(recover_camera_dir) = recover_camera_dir.as_ref() {
        ensure_dir(recover_camera_dir);
    }

    Ok((output_camera_dir, recover_camera_dir))
}

// Starts the background thread that writes frame metadata sidecars.
fn start_metadata_writer(
    output_camera_dir: &Path,
) -> (
    mpsc::Sender<FrameMetadata>,
    thread::JoinHandle<()>,
) {
    let (metadata_tx, metadata_rx) = mpsc::channel::<FrameMetadata>();
    let metadata_output_dir = output_camera_dir.to_path_buf();
    let metadata_writer = thread::spawn(move || {
        while let Ok(metadata) = metadata_rx.recv() {
            write_frame_metadata_file(&metadata_output_dir, metadata.frame_index, &metadata);
        }
    });
    (metadata_tx, metadata_writer)
}

// Records frames from one camera into one lossless H.265 stream.
pub fn record_h265_from_one_camera(
    config: &CameraIngestConfig,
    output_base_dir: &Path,
    recover_base_dir: Option<&Path>,
    max_frames: Option<usize>,
    max_duration: Option<f64>,
) -> Result<()> {
    config.validate().map_err(anyhow::Error::msg)?;

    let (output_camera_dir, recover_camera_dir) =
        prepare_camera_dirs(&config.camera_id, output_base_dir, recover_base_dir)?;

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
    let mut encoder = H265CameraEncoder::new(
        &output_camera_dir,
        &config.camera_id,
        width_u32,
        height_u32,
        config.frame_rate_hz,
    )?;
    let (metadata_tx, metadata_writer) = start_metadata_writer(&output_camera_dir);

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
                    let metadata = FrameMetadata {
                        camera_id: config.camera_id.clone(),
                        frame_index: frames_written as usize,
                        width,
                        height,
                        payload_bytes: data.len(),
                        system_timestamp_ns: buffer.system_timestamp(),
                        buffer_timestamp_ns: buffer.timestamp(),
                        frame_id: buffer.frame_id(),
                        exposure_time_us: config.exposure_time_us,
                        frame_rate_hz: config.frame_rate_hz,
                    };
                    metadata_tx
                        .send(metadata)
                        .context("queue compressed frame metadata write")?;

                    frames_written += 1;

                    if frames_written % 10 == 0 {
                        println!(
                            "Camera {}: encoded {} frame(s) into one lossless H.265 stream",
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
    drop(metadata_tx);
    metadata_writer
        .join()
        .map_err(|_| anyhow::Error::msg("metadata writer thread panicked"))?;
    recording_result?;

    let summary: H265SessionSummary = encoder.finish()?;
    println!(
        "Finished camera {}. Wrote {} frame(s) into {}",
        config.camera_id,
        summary.frames_written,
        summary.h265_path.display()
    );

    if let Some(recover_camera_dir) = recover_camera_dir.as_ref() {
        let recovery = recover_h265_to_png(&summary.h265_path, recover_camera_dir)?;
        println!(
            "Recovered {} PNG frame(s) for camera {} into {}",
            recovery.frames_recovered,
            config.camera_id,
            recover_camera_dir.display()
        );
    }

    Ok(())
}
