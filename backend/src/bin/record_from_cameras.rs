use clap::Parser;
use crossbeam::channel::unbounded;
use mjolnir::camera_ingest::ingest_frames;
use mjolnir::cli::{Cli, Commands};
use mjolnir::schemas::camera_ingest_config::CameraIngestConfig;

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::RecordFromCameras(args) => {
            for camera in args.cameras {
                let (tx, _rx) = unbounded();

                let config = CameraIngestConfig {
                    device_id: camera,
                    exposure_time_us: args.exposure_us,
                    frame_rate_hz: args.frame_rate_hz,
                    aperture: args.aperture,
                    enable_ptp: false,
                    use_fake_interface: args.use_fake_interface,
                    num_buffers: args.num_buffers,
                    timeout_ms: args.timeout_ms,
                }
                .validate()
                .expect("invalid camera ingest config");

                ingest_frames(tx, config);
            }
        }
        Commands::DiscoverCameras => {
            eprintln!("use //backend:discover_cameras for camera discovery");
        }
    }
}