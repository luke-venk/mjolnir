/// Tool for users to record footage from the cameras using Aravis and
/// store the frames to disk using lossless H.265 from the command-line.
use backend_lib::camera::CameraIngestConfig;
use backend_lib::camera::aravis_utils::initialize_aravis;
use backend_lib::camera::record::cli::RecordFromCamerasArgs;
use backend_lib::camera::record::writer::{ensure_dir, string_to_pathbuf};
use backend_lib::camera_ingest::{
    ensure_ffmpeg_lossless_hevc_support, record_h265_from_one_camera,
};

use clap::Parser;

pub fn main() {
    println!("------------------------");
    println!("RECORDING FROM CAMERA...");
    println!("------------------------\n");

    // Store command line arguments for recording.
    let args: RecordFromCamerasArgs = RecordFromCamerasArgs::parse();
    args.validate().unwrap_or_else(|err| panic!("{err}"));

    // Confirm ffmpeg can produce lossless H.265 before touching the camera.
    ensure_ffmpeg_lossless_hevc_support().unwrap_or_else(|err| panic!("{err:#}"));
    let _aravis = initialize_aravis();

    // Create output directory based on command-line argument.
    let output_base_dir = string_to_pathbuf(&args.output_dir);
    ensure_dir(&output_base_dir);

    let recover_base_dir = args.recover_to_png_dir.as_ref().map(string_to_pathbuf);
    if let Some(recover_base_dir) = recover_base_dir.as_ref() {
        ensure_dir(recover_base_dir);
    }

    // Parse command line arguments into camera ingest config.
    let camera_ingest_config: CameraIngestConfig = CameraIngestConfig::from_record_args(args.clone());
    camera_ingest_config
        .validate()
        .unwrap_or_else(|err| panic!("{err}"));

    // Begin recording into one lossless H.265 stream, then optionally recover it to PNGs.
    record_h265_from_one_camera(
        &camera_ingest_config,
        &output_base_dir,
        recover_base_dir.as_deref(),
        args.max_frames,
        args.max_duration,
    )
    .unwrap_or_else(|err| panic!("{err:#}"));
}

#[cfg(test)]
mod tests {
    use backend_lib::camera::record::cli::RecordFromCamerasArgs;
    use backend_lib::camera::{CameraIngestConfig, Resolution};
    use clap::Parser;

    #[test]
    fn resolution_dimensions_match_expected_values() {
        assert_eq!(Resolution::HD.dimensions(), (1024, 750));
        assert_eq!(Resolution::FullHD.dimensions(), (2048, 1500));
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
            restart_requested: false,
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
            restart_requested: false,
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
            restart_requested: false,
        };
        assert!(bad_buffers.validate().is_err());
    }

    
    #[test]
    fn cli_parses_recovery_flag_and_alias_output_dir() {
        let args = RecordFromCamerasArgs::try_parse_from([
            "record_from_cameras",
            "--camera",
            "cam-a",
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

        assert_eq!(args.camera_id, "cam-a");
        assert_eq!(args.output_dir, "/tmp/out");
        assert_eq!(args.recover_to_png_dir.as_deref(), Some("/tmp/png"));
        assert_eq!(args.max_duration, Some(1.5));
        assert!(matches!(args.resolution, Resolution::UHD4K));
        assert_eq!(args.frame_rate_hz, 30.0);
    }
}
