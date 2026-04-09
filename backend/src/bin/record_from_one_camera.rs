/// Tool for users to record footage from one camera using Aravis and
/// store the frames to disk using the command-line.
use backend_lib::camera::CameraIngestConfig;
use backend_lib::camera::aravis_utils::initialize_aravis;
use backend_lib::camera::record::cli::RecordWithOneCameraArgs;
use backend_lib::camera::record::compression::ensure_ffmpeg_lossless_hevc_support;
use backend_lib::camera::record::record_from_one_camera;
use backend_lib::camera::record::writer::{ensure_dir, string_to_pathbuf};

use clap::Parser;

pub fn main() {
    println!("------------------------");
    println!("RECORDING FROM ONE CAMERA...");
    println!("------------------------\n");

    // Store command line arguments for recording.
    let args: RecordWithOneCameraArgs = RecordWithOneCameraArgs::parse();
    args.common_args
        .validate()
        .unwrap_or_else(|err| panic!("{err}"));

    if args.common_args.compress {
        ensure_ffmpeg_lossless_hevc_support().unwrap_or_else(|err| panic!("{err:#}"));
    }

    // Create output directory based on command-line argument.
    let output_base_dir = string_to_pathbuf(&args.common_args.output_dir);
    ensure_dir(&output_base_dir);

    let recover_base_dir = args
        .common_args
        .recover_to_png_dir
        .as_ref()
        .map(string_to_pathbuf);
    if let Some(recover_base_dir) = recover_base_dir.as_ref() {
        ensure_dir(recover_base_dir);
    }

    // Parse command line arguments into camera ingest config.
    let camera_ingest_config: CameraIngestConfig = CameraIngestConfig::from_record_one_args(args.clone());
    camera_ingest_config
        .validate()
        .unwrap_or_else(|err| panic!("{err}"));

    let _aravis = initialize_aravis();

    // Begin recording, optionally routing through the lossless H.265 helper.
    record_from_one_camera(
        &camera_ingest_config,
        &output_base_dir,
        recover_base_dir.as_ref(),
        args.common_args.max_frames,
        args.common_args.max_duration,
    );
}

#[cfg(test)]
mod tests {
    use backend_lib::camera::record::cli::RecordWithOneCameraArgs;
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
            compress: true,
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
            compress: true,
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
            compress: true,
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
    fn cli_parses_recovery_flag_and_output_alias() {
        let args = RecordWithOneCameraArgs::try_parse_from([
            "record_from_one_camera",
            "--camera",
            "cam-a",
            "--save-recordings-dir",
            "/tmp/out",
            "--recover-to-png-dir",
            "/tmp/png",
            "--compress",
            "true",
            "--max-duration",
            "1.5",
            "--resolution",
            "4k",
            "--frame-rate-hz",
            "30",
        ])
        .expect("CLI args should parse");

        assert_eq!(args.camera_id, "cam-a");
        assert_eq!(args.common_args.output_dir, "/tmp/out");
        assert_eq!(args.common_args.recover_to_png_dir.as_deref(), Some("/tmp/png"));
        assert!(args.common_args.compress);
        assert_eq!(args.common_args.max_duration, Some(1.5));
        assert!(matches!(args.common_args.resolution, Resolution::UHD4K));
        assert_eq!(args.common_args.frame_rate_hz, 30.0);
    }
}
