use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use std::slice;
use std::sync::Once;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use aravis::prelude::*;
use aravis::{Aravis, Buffer, BufferStatus, Camera, Stream};
use clap::{Parser, ValueEnum};
use serde::Serialize;
use serde_json::to_string_pretty;

static ARAVIS_INIT: Once = Once::new();

// TODO: this file is messy, reorganize into several files.

/// The command line arguments we'd expect for the cameras to record.
#[derive(Parser, Debug, Clone)]
#[command(name = "record_from_cameras_raw")]
#[command(about = "Records raw frames from Aravis camera(s) into an output directory.")]
pub struct RecordFromCamerasArgs {
    #[arg(long = "camera", required = true)]
    pub cameras: Vec<String>,

    /// Exposure time in microseconds.
    #[arg(long = "exposure-us", default_value_t = 100.0)]
    pub exposure_us: f64,

    #[arg(long = "frame-rate-hz", default_value_t = 30.0)]
    pub frame_rate_hz: f64,

    #[arg(long, value_enum, default_value_t = Resolution::UHD4K)]
    pub resolution: Resolution,

    /// Optional lens iris feature value. Only applied if the device exposes an Iris feature.
    #[arg(long)]
    pub aperture: Option<f64>,

    #[arg(long, default_value_t = 16)]
    pub num_buffers: usize,

    /// Timeout for waiting on a frame buffer, in milliseconds.
    #[arg(long, default_value_t = 200)]
    pub timeout_ms: u64,

    /// Output directory where frames will be written to.
    #[arg(long)]
    pub save_recordings_dir: String,

    /// Stop recording after this many frames per camera.
    #[arg(long)]
    pub max_frames: Option<usize>,

    /// Stop recording after this many seconds.
    #[arg(long)]
    pub max_duration: Option<f64>,

    /// Whether to enable Precision Time Protocol if supported by the device.
    #[arg(long, default_value_t = false)]
    pub enable_ptp: bool,
}

/// Different resolutions we might want to record with.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Resolution {
    #[value(name = "720p")]
    HD,
    #[value(name = "1080p")]
    FullHD,
    // Atlas capture uses the full 12.3 MP mono frame even though the CLI flag stays `4k`.
    #[value(name = "4k")]
    UHD4K,
}

impl Resolution {
    pub fn dimensions(&self) -> (i32, i32) {
        match self {
            Resolution::HD => (1280, 720),
            Resolution::FullHD => (1920, 1080),
            Resolution::UHD4K => (4096, 3000),
        }
    }
}

/// Configuration for what specs we want to use while recording.
// TODO: adjust this based on what discover_cameras.rs actually returns.
#[derive(Debug, Clone)]
pub struct CameraIngestConfig {
    pub device_id: String,

    // Core capture settings.
    pub exposure_time_us: f64,
    pub frame_rate_hz: f64,
    pub resolution: Resolution,

    // Optional / hardware-dependent.
    pub aperture: Option<f64>,

    // System-level config.
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

    pub fn validate(&self) -> Result<(), String> {
        if self.device_id.is_empty() {
            return Err("device_id cannot be empty".to_string());
        }
        if self.exposure_time_us <= 0.0 {
            return Err("exposure_time_us must be > 0".to_string());
        }
        if self.frame_rate_hz <= 0.0 {
            return Err("frame_rate_hz must be > 0".to_string());
        }
        if self.num_buffers == 0 {
            return Err("num_buffers must be > 0".to_string());
        }
        Ok(())
    }
}

/// Metadata for each frame to be recorded in addition to raw files.
#[derive(Debug, Serialize)]
struct FrameMetadata {
    device_id: String,
    frame_index: usize,
    width: i32,
    height: i32,
    payload_bytes: usize,
    system_timestamp_ns: u64,
    buffer_timestamp_ns: u64,
    frame_id: u64,
    exposure_time_us: f64,
    frame_rate_hz: f64,
}

/// Initializes Aravis once in a thread-safe manner?
fn initialize_aravis() {
    ARAVIS_INIT.call_once(|| {
        Aravis::initialize().expect("Failed to initialize Aravis.");
    });
}

/// Use Aravis to open each camera.
fn open_camera(device_id: &str) -> Camera {
    Camera::new(Some(device_id))
        .unwrap_or_else(|_| panic!("ERROR: Failed to open camera with device_id={device_id}"))
}

/// Loads camera configuration into Aravis library camera configuration.
fn configure_camera(camera: &Camera, config: &CameraIngestConfig) {
    camera
        .set_exposure_time(config.exposure_time_us)
        .expect("Failed to set exposure time in camera configuration.");
    camera
        .set_frame_rate(config.frame_rate_hz)
        .expect("Failed to set frame rate in camera configuration.");

    let (width, height) = config.resolution.dimensions();
    camera
        .set_region(0, 0, width, height)
        .expect("Failed to set resolution in camera configuration.");

    if config.enable_ptp {
        camera.set_boolean("PtpEnable", true).expect("Failed to enable ptp");
    }

    camera.gv_set_packet_size(8064).expect("err auto packet size");
    camera.gv_set_packet_delay(5000).expect("err auto packet delay");
    println!("Packet size: {}", camera.gv_get_packet_size().expect("fdsafsd"));

    camera.set_pixel_format(aravis::PixelFormat::MONO_8).expect("err");
    camera.set_gain(0.0).expect("err");
    camera.set_frame_rate_enable(true).expect("err");
}

/// Creates Aravis camera stream and allocates frame buffers.
fn create_stream_and_queue_buffers(camera: &Camera, num_buffers: usize) -> Stream {
    let stream = camera
        .create_stream()
        .expect("Failed to create camera stream.");

    let payload_size = camera
        .payload()
        .expect("Failed to get camera payload size.");

    for _ in 0..num_buffers {
        let buffer = Buffer::new_allocate(payload_size as usize);
        stream.push_buffer(buffer);
    }

    stream
}

/// Converts an Aravis buffer into a vector of bytes.
fn copy_buffer_bytes(buffer: &Buffer) -> Vec<u8> {
    let (ptr, len) = buffer.data();

    if ptr.is_null() || len == 0 {
        return Vec::new();
    }

    unsafe { slice::from_raw_parts(ptr as *const u8, len).to_vec() }
}

/// Helper function to format timestamp string.
fn timestamp_string() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before UNIX_EPOCH");
    format!("{}_{}", now.as_secs(), now.subsec_nanos())
}

/// Helper function to ensure output directory exists.
fn ensure_dir(path: &PathBuf) {
    fs::create_dir_all(path)
        .unwrap_or_else(|e| panic!("failed to create directory {}: {e}", path.display()));
}

/// Helper function to clean string values.
fn sanitize(value: &str) -> String {
    value
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn write_frame_files(
    output_dir: &PathBuf,
    camera_id: &str,
    frame_index: usize,
    data: &[u8],
    metadata: &FrameMetadata,
) {
    let basename = format!(
        "{}_frame_{:06}_{}",
        sanitize(camera_id),
        frame_index,
        timestamp_string()
    );

    let raw_path = output_dir.join(format!("{basename}.raw"));
    let json_path = output_dir.join(format!("{basename}.json"));

    let mut raw_file = File::create(&raw_path)
        .unwrap_or_else(|e| panic!("failed to create {}: {e}", raw_path.display()));
    raw_file
        .write_all(data)
        .unwrap_or_else(|e| panic!("failed to write {}: {e}", raw_path.display()));

    let json = to_string_pretty(metadata).expect("failed to serialize frame metadata");
    let mut json_file = File::create(&json_path)
        .unwrap_or_else(|e| panic!("failed to create {}: {e}", json_path.display()));
    json_file
        .write_all(json.as_bytes())
        .unwrap_or_else(|e| panic!("failed to write {}: {e}", json_path.display()));
}

fn record_one_camera(
    config: &CameraIngestConfig,
    output_base_dir: &PathBuf,
    max_frames: Option<usize>,
    max_duration: Option<f64>,
) {
    config
        .validate()
        .expect("Invalid camera ingest configuration.");

    let camera_dir = output_base_dir.join(sanitize(&config.device_id));
    ensure_dir(&camera_dir);

    println!("-------------------------");
    println!("Opening cameras");
    println!("-------------------------");
    println!("Opening camera {}\n", config.device_id);
    let camera = open_camera(&config.device_id);
    configure_camera(&camera, config);
    let stream = create_stream_and_queue_buffers(&camera, config.num_buffers);

    let (_, _, width, height) = camera
        .region()
        .expect("Failed to read camera region after configuration.");
    let payload = camera
        .payload()
        .expect("failed to read payload after configuration");
    println!(
        "Configured camera {}: width={} height={} payload={} exposure_us={} frame_rate_hz={}",
        config.device_id, width, height, payload, config.exposure_time_us, config.frame_rate_hz
    );

    camera
        .start_acquisition()
        .expect("Failed to start acquisition.");

    let start = Instant::now();
    let mut frames_saved = 0usize;

    loop {
        if let Some(limit) = max_frames {
            if frames_saved >= limit {
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

                let metadata = FrameMetadata {
                    device_id: config.device_id.clone(),
                    frame_index: frames_saved,
                    width,
                    height,
                    payload_bytes: data.len(),
                    system_timestamp_ns: buffer.system_timestamp(),
                    buffer_timestamp_ns: buffer.timestamp(),
                    frame_id: buffer.frame_id(),
                    exposure_time_us: config.exposure_time_us,
                    frame_rate_hz: config.frame_rate_hz,
                };

                write_frame_files(
                    &camera_dir,
                    &config.device_id,
                    frames_saved,
                    &data,
                    &metadata,
                );

                frames_saved += 1;

                if frames_saved % 10 == 0 {
                    println!(
                        "Camera {}: saved {} frame(s)",
                        config.device_id, frames_saved
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

    let _ = camera.stop_acquisition();

    println!(
        "Finished camera {}. Saved {} frame(s) into {}",
        config.device_id,
        frames_saved,
        camera_dir.display()
    );
}

pub fn main() {
    println!("-------------------------");
    println!("RECORDING FROM CAMERAS...");
    println!("-------------------------\n");

    println!("-----------------------------");
    println!("Camera ingest configurations:");
    println!("-----------------------------\n");
    let args: RecordFromCamerasArgs = RecordFromCamerasArgs::parse();

    if args.max_frames.is_none() && args.max_duration.is_none() {
        panic!("You must provide at least one stopping condition: --max-frames or --max-duration")
    }

    initialize_aravis();
    let output_dir = PathBuf::from(&args.save_recordings_dir);
    ensure_dir(&output_dir);

    for camera_id in &args.cameras {
        let camera_ingest_config: CameraIngestConfig = CameraIngestConfig::new(
            camera_id.clone(),
            args.exposure_us,
            args.frame_rate_hz,
            args.resolution,
            args.aperture,
            args.enable_ptp,
            args.num_buffers,
            args.timeout_ms,
        );

        camera_ingest_config
            .validate()
            .expect("Invalid camera ingest configuration.");
        println!("{camera_ingest_config:#?}\n");

        record_one_camera(
            &camera_ingest_config,
            &output_dir,
            args.max_frames,
            args.max_duration,
        );
    }
}
