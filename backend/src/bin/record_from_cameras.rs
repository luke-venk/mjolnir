use std::fs;
use std::path::{Path, PathBuf};
use std::slice;
use std::sync::Once;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use aravis::prelude::*;
use aravis::{Aravis, Buffer, BufferStatus, Camera, Stream};
use backend_lib::camera_ingest::{
    H265CameraEncoder, ensure_ffmpeg_lossless_hevc_support, recover_h265_to_png, sanitize,
};
use clap::{Parser, ValueEnum};

//was using this command for testing with fake monostream: 
//ffmpeg -f lavfi -i testsrc=duration=10:size=3840x2140:rate=30 -pix_fmt gray output_mono8.mp4



static ARAVIS_INIT: Once = Once::new();

#[derive(Parser, Debug, Clone)]
#[command(name = "record_from_cameras")]
#[command(about = "Records Aravis camera frames into one lossless H.265 stream per camera.")]
pub struct RecordFromCamerasArgs {
    #[arg(long = "camera", required = true)]
    pub cameras: Vec<String>,

    #[arg(long = "exposure-us", default_value_t = 100.0)]
    pub exposure_us: f64,

    #[arg(long = "frame-rate-hz", default_value_t = 30.0)]
    pub frame_rate_hz: f64,

    #[arg(long, value_enum, default_value_t = Resolution::UHD4K)]
    pub resolution: Resolution,

    #[arg(long)]
    pub aperture: Option<f64>,

    #[arg(long, default_value_t = 16)]
    pub num_buffers: usize,

    #[arg(long, default_value_t = 200)]
    pub timeout_ms: u64,

    #[arg(long)]
    pub save_recordings_dir: String,

    #[arg(long)]
    pub recover_to_png_dir: Option<String>,

    #[arg(long)]
    pub max_frames: Option<usize>,

    #[arg(long)]
    pub max_duration: Option<f64>,

    #[arg(long, default_value_t = false)]
    pub enable_ptp: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Resolution {
    #[value(name = "720p")]
    HD,
    #[value(name = "1080p")]
    FullHD,
    #[value(name = "4k")]
    UHD4K,
}

impl Resolution {
    pub fn dimensions(&self) -> (i32, i32) {
        match self {
            Resolution::HD => (1280, 720),
            Resolution::FullHD => (1920, 1080),
            Resolution::UHD4K => (3840, 2160),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CameraIngestConfig {
    pub device_id: String,
    pub exposure_time_us: f64,
    pub frame_rate_hz: f64,
    pub resolution: Resolution,
    pub aperture: Option<f64>,
    pub enable_ptp: bool,
    pub num_buffers: usize,
    pub timeout_ms: u64,
}

impl CameraIngestConfig {
    pub fn new(
        device_id: String,
        exposure_time_us: f64,
        frame_rate_hz: f64,
        resolution: Resolution,
        aperture: Option<f64>,
        enable_ptp: bool,
        num_buffers: usize,
        timeout_ms: u64,
    ) -> Self {
        Self {
            device_id,
            exposure_time_us,
            frame_rate_hz,
            resolution,
            aperture,
            enable_ptp,
            num_buffers,
            timeout_ms,
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.device_id.is_empty() {
            anyhow::bail!("device_id cannot be empty");
        }
        if self.exposure_time_us <= 0.0 {
            anyhow::bail!("exposure_time_us must be > 0");
        }
        if self.frame_rate_hz <= 0.0 {
            anyhow::bail!("frame_rate_hz must be > 0");
        }
        if self.num_buffers == 0 {
            anyhow::bail!("num_buffers must be > 0");
        }
        Ok(())
    }
}

fn initialize_aravis() {
    ARAVIS_INIT.call_once(|| {
        Aravis::initialize().expect("Failed to initialize Aravis.");
    });
}

fn open_camera(device_id: &str) -> Camera {
    Camera::new(Some(device_id))
        .unwrap_or_else(|_| panic!("ERROR: Failed to open camera with device_id={device_id}"))
}

fn configure_camera(camera: &Camera, config: &CameraIngestConfig) -> Result<()> {
    camera
        .set_exposure_time(config.exposure_time_us)
        .context("set exposure time")?;
    camera
        .set_frame_rate(config.frame_rate_hz)
        .context("set frame rate")?;

    let (width, height) = config.resolution.dimensions();
    camera
        .set_region(0, 0, width, height)
        .context("set camera region")?;

    if let Some(aperture) = config.aperture {
        if let Err(error) = camera.set_float("Iris", aperture) {
            eprintln!(
                "Camera {} does not accept Iris={aperture}: {error}",
                config.device_id
            );
        }
    }

    if config.enable_ptp {
        camera
            .set_boolean("PtpEnable", true)
            .context("enable PTP")?;
    }

    camera
        .gv_set_packet_size(8064)
        .context("set packet size")?;
    camera
        .gv_set_packet_delay(5000)
        .context("set packet delay")?;
    // Atlas frames are captured as 8-bit monochrome so the encoder sees MONO_8 / gray bytes.
    camera
        .set_pixel_format(aravis::PixelFormat::MONO_8)
        .context("set MONO_8 pixel format")?;
    camera.set_gain(0.0).context("set gain")?;
    camera
        .set_frame_rate_enable(true)
        .context("enable frame rate control")?;

    Ok(())
}

fn create_stream_and_queue_buffers(camera: &Camera, num_buffers: usize) -> Result<Stream> {
    let stream = camera.create_stream().context("create camera stream")?;
    let payload_size = camera.payload().context("read camera payload size")?;

    for _ in 0..num_buffers {
        let buffer = Buffer::new_allocate(payload_size as usize);
        stream.push_buffer(buffer);
    }

    Ok(stream)
}

fn copy_buffer_bytes(buffer: &Buffer) -> Vec<u8> {
    let (ptr, len) = buffer.data();

    if ptr.is_null() || len == 0 {
        return Vec::new();
    }

    unsafe { slice::from_raw_parts(ptr as *const u8, len).to_vec() }
}

fn ensure_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path).with_context(|| format!("create directory {}", path.display()))
}

fn record_one_camera(
    config: &CameraIngestConfig,
    output_base_dir: &Path,
    recover_base_dir: Option<&Path>,
    max_frames: Option<usize>,
    max_duration: Option<f64>,
) -> Result<()> {
    config.validate()?;

    let camera_dir = output_base_dir.join(sanitize(&config.device_id));
    ensure_dir(&camera_dir)?;

    println!("-------------------------");
    println!("Opening camera {}", config.device_id);
    println!("-------------------------");

    let camera = open_camera(&config.device_id);
    configure_camera(&camera, config)?;
    let stream = create_stream_and_queue_buffers(&camera, config.num_buffers)?;

    let (_, _, width, height) = camera
        .region()
        .context("read camera region after configuration")?;
    let payload = camera
        .payload()
        .context("read camera payload after configuration")?;

    println!(
        "Configured camera {}: width={} height={} payload={} exposure_us={} frame_rate_hz={}",
        config.device_id, width, height, payload, config.exposure_time_us, config.frame_rate_hz
    );

    let width_u32 = u32::try_from(width).context("camera width does not fit into u32")?;
    let height_u32 = u32::try_from(height).context("camera height does not fit into u32")?;
    // Create one long-lived lossless H.265 encoder for this camera instead of writing raw frames.
    let mut encoder = H265CameraEncoder::new(
        &camera_dir,
        &config.device_id,
        width_u32,
        height_u32,
        config.frame_rate_hz,
    )?;

    camera
        .start_acquisition()
        .context("start camera acquisition")?;

    let start = Instant::now();
    let mut frames_written = 0u64;
    let recording_result: Result<()> = (|| {
        loop {
            if let Some(limit) = max_frames {
                if frames_written >= limit as u64 {
                    break;
                }
            }

            if let Some(seconds) = max_duration {
                if start.elapsed() >= Duration::from_secs_f64(seconds) {
                    break;
                }
            }

            let buffer = match stream.timeout_pop_buffer(config.timeout_ms) {
                Some(buffer) => buffer,
                None => {
                    eprintln!(
                        "Timeout waiting for buffer from camera {}",
                        config.device_id
                    );
                    continue;
                }
            };

            match buffer.status() {
                BufferStatus::Success => {
                    let data = copy_buffer_bytes(&buffer);

                    if data.is_empty() {
                        eprintln!("Empty buffer from camera {}.", config.device_id);
                        stream.push_buffer(buffer);
                        continue;
                    }

                    encoder.push_frame(&data)?;
                    frames_written += 1;

                    if frames_written % 10 == 0 {
                        println!(
                            "Camera {}: encoded {} frame(s) to lossless H.265",
                            config.device_id, frames_written
                        );
                    }
                }
                status => {
                    eprintln!(
                        "Camera {} returned non-success buffer status: {:?}",
                        config.device_id, status
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
        config.device_id,
        summary.frames_written,
        summary.h265_path.display()
    );

    if let Some(recover_base_dir) = recover_base_dir {
        let png_dir = recover_base_dir.join(sanitize(&config.device_id));
        // Optional same-command recovery: decode the recorded H.265 stream back into PNG frames.
        let recovery = recover_h265_to_png(&summary.h265_path, &png_dir)?;
        println!(
            "Recovered {} PNG frame(s) for camera {} into {}",
            recovery.frames_recovered,
            config.device_id,
            recovery.output_dir.display()
        );
    }

    Ok(())
}

fn run() -> Result<()> {
    println!("-------------------------------");
    println!("LOSSLESS H.265 CAMERA RECORDING");
    println!("-------------------------------\n");

    let args = RecordFromCamerasArgs::parse();

    if args.max_frames.is_none() && args.max_duration.is_none() {
        anyhow::bail!(
            "You must provide at least one stopping condition: --max-frames or --max-duration"
        );
    }

    ensure_ffmpeg_lossless_hevc_support()?;
    initialize_aravis();

    let output_dir = PathBuf::from(&args.save_recordings_dir);
    ensure_dir(&output_dir)?;
    let recover_dir = args.recover_to_png_dir.as_ref().map(PathBuf::from);
    if let Some(recover_dir) = recover_dir.as_ref() {
        ensure_dir(recover_dir)?;
    }

    for camera_id in &args.cameras {
        let config = CameraIngestConfig::new(
            camera_id.clone(),
            args.exposure_us,
            args.frame_rate_hz,
            args.resolution,
            args.aperture,
            args.enable_ptp,
            args.num_buffers,
            args.timeout_ms,
        );

        println!("{config:#?}\n");
        record_one_camera(
            &config,
            &output_dir,
            recover_dir.as_deref(),
            args.max_frames,
            args.max_duration,
        )?;
    }

    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("record_from_cameras failed: {error:#}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::{CameraIngestConfig, RecordFromCamerasArgs, Resolution};
    use clap::Parser;

    #[test]
    fn resolution_dimensions_match_expected_values() {
        assert_eq!(Resolution::HD.dimensions(), (1280, 720));
        assert_eq!(Resolution::FullHD.dimensions(), (1920, 1080));
        assert_eq!(Resolution::UHD4K.dimensions(), (3840, 2160));
    }

    #[test]
    fn camera_ingest_config_validate_rejects_invalid_values() {
        let empty_id = CameraIngestConfig::new(
            String::new(),
            100.0,
            30.0,
            Resolution::UHD4K,
            None,
            false,
            16,
            200,
        );
        assert!(empty_id.validate().is_err());

        let bad_exposure = CameraIngestConfig::new(
            "cam-a".to_string(),
            0.0,
            30.0,
            Resolution::UHD4K,
            None,
            false,
            16,
            200,
        );
        assert!(bad_exposure.validate().is_err());

        let bad_buffers = CameraIngestConfig::new(
            "cam-a".to_string(),
            100.0,
            30.0,
            Resolution::UHD4K,
            None,
            false,
            0,
            200,
        );
        assert!(bad_buffers.validate().is_err());
    }

    #[test]
    fn cli_parses_recovery_flag_and_limits() {
        let args = RecordFromCamerasArgs::try_parse_from([
            "record_from_cameras",
            "--camera",
            "cam-a",
            "--camera",
            "cam-b",
            "--save-recordings-dir",
            "/tmp/out",
            "--recover-to-png-dir",
            "/tmp/png",
            "--max-duration",
            "1.5",
            "--resolution",
            "4k",
            "--frame-rate-hz",
            "30",
        ])
        .expect("CLI args should parse");

        assert_eq!(args.cameras, vec!["cam-a", "cam-b"]);
        assert_eq!(args.save_recordings_dir, "/tmp/out");
        assert_eq!(args.recover_to_png_dir.as_deref(), Some("/tmp/png"));
        assert_eq!(args.max_duration, Some(1.5));
        assert!(matches!(args.resolution, Resolution::UHD4K));
        assert_eq!(args.frame_rate_hz, 30.0);
    }
}
