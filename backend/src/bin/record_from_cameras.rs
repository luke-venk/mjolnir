use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use backend_lib::camera::aravis_utils::initialize_aravis;
use backend_lib::camera::{CameraIngestConfig, Resolution};
use backend_lib::camera_ingest::{
    ensure_ffmpeg_lossless_hevc_support, record_h265_from_one_camera,
};
use clap::Parser;

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

impl RecordFromCamerasArgs {
    fn validate(&self) -> Result<()> {
        if self.max_frames.is_none() && self.max_duration.is_none() {
            anyhow::bail!(
                "You must provide at least one stopping condition: --max-frames or --max-duration"
            );
        }

        if self.num_buffers == 0 {
            anyhow::bail!("num_buffers must be > 0");
        }

        Ok(())
    }

    fn camera_config_for(&self, camera_id: &str) -> CameraIngestConfig {
        CameraIngestConfig {
            camera_id: camera_id.to_string(),
            exposure_time_us: self.exposure_us,
            frame_rate_hz: self.frame_rate_hz,
            resolution: self.resolution,
            aperture: self.aperture,
            enable_ptp: self.enable_ptp,
            num_buffers: self.num_buffers,
            timeout_ms: self.timeout_ms,
        }
    }
}

fn ensure_dir(path: &PathBuf) -> Result<()> {
    fs::create_dir_all(path).with_context(|| format!("create directory {}", path.display()))
}

fn run() -> Result<()> {
    println!("-------------------------------");
    println!("LOSSLESS H.265 CAMERA RECORDING");
    println!("-------------------------------\n");

    let args = RecordFromCamerasArgs::parse();
    args.validate()?;

    ensure_ffmpeg_lossless_hevc_support()?;
    let _aravis = initialize_aravis();

    let output_base_dir = PathBuf::from(&args.save_recordings_dir);
    ensure_dir(&output_base_dir)?;

    let recover_base_dir = args.recover_to_png_dir.as_ref().map(PathBuf::from);
    if let Some(recover_base_dir) = recover_base_dir.as_ref() {
        ensure_dir(recover_base_dir)?;
    }

    for camera_id in &args.cameras {
        let config = args.camera_config_for(camera_id);
        config.validate().map_err(anyhow::Error::msg)?;
        println!("{config:#?}\n");

        record_h265_from_one_camera(
            &config,
            &output_base_dir,
            recover_base_dir.as_deref(),
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
    use super::RecordFromCamerasArgs;
    use backend_lib::camera::{CameraIngestConfig, Resolution};
    use clap::Parser;

    #[test]
    fn resolution_dimensions_match_expected_values() {
        assert_eq!(Resolution::HD.dimensions(), (1280, 720));
        assert_eq!(Resolution::FullHD.dimensions(), (1920, 1080));
        assert_eq!(Resolution::UHD4K.dimensions(), (4096, 3000));
    }

    #[test]
    fn camera_ingest_config_validate_rejects_invalid_values() {
        let empty_id = CameraIngestConfig {
            camera_id: String::new(),
            exposure_time_us: 100.0,
            frame_rate_hz: 30.0,
            resolution: Resolution::UHD4K,
            aperture: None,
            enable_ptp: false,
            num_buffers: 16,
            timeout_ms: 200,
        };
        assert!(empty_id.validate().is_err());

        let bad_exposure = CameraIngestConfig {
            camera_id: "cam-a".to_string(),
            exposure_time_us: 0.0,
            frame_rate_hz: 30.0,
            resolution: Resolution::UHD4K,
            aperture: None,
            enable_ptp: false,
            num_buffers: 16,
            timeout_ms: 200,
        };
        assert!(bad_exposure.validate().is_err());

        let bad_buffers = CameraIngestConfig {
            camera_id: "cam-a".to_string(),
            exposure_time_us: 100.0,
            frame_rate_hz: 30.0,
            resolution: Resolution::UHD4K,
            aperture: None,
            enable_ptp: false,
            num_buffers: 0,
            timeout_ms: 200,
        };
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
