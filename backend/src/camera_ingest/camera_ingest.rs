//defines how cam settigs should be stored and read from env variables
// reads cameras settings from env variables, sends placeholer frames
use crate::schemas::{Frame, Context};
use std::env;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::Duration;

//use aravis::prelude::*;
use crate::hardware::{Context, Frame};
use crate::hardware::camera::CameraId;

//maps cameras (first camera ingest = left, next is right)
static CAMERA_INSTANCE_COUNT: AtomicUsize = AtomicUsize::new(0);

//number of image buffers
const DEFAULT_NUM_BUFFERS: usize = 16;
//timeout waiting for frame
const DEFAULT_TIMEOUT_MS: u64 = 200;

//config 


//using cameraslot as a helper inside this
// first call assigned to slot left, second call to camera_slot right
//did it like this as it was done left and then right in main.rs as well
#[derive(Debug, Clone, Copy)]
enum CameraSlot {
    Left, 
    Right,
}


impl CameraSlot{
    fn next() -> Self{
        match CAMERA_INSTANCE_COUNT.fetch_add(1, Ordering::SeqCst){
            0 => Self::Left, 
            _ => Self::Right,
        }
    }


    //returns env variable for camera slot
    fn prefix(&self) -> &'static str {
        match self {
            Self::Left => "FIELD_LEFT",
            Self::Right => "FIELD_RIGHT",
        }
    }


    //
    fn camera_id(&self) -> CameraId{
        match self {
            Self::Left => CameraId:: FieldLeft,
            Self::Right => CameraId:: FieldRight,
        }
    }


    //just states which camera embeds frame (left then right)
    fn tag(&self) -> u64{
        match self{
            Self::Left => 1,
            Self::Right => 2, 
        }
    }
}

#[derive(Debug, Clone)]
pub struct CameraIngestConfig{
    pub device_id: Option<String>, // for our camera id
    pub exposure_time_us: Option<f64>, // based on specs
    pub frame_rate_hz: Option<f64>, //max intake of 45.2 fps needed
    pub aperture: Option<f64>,
    pub enable_ptp: bool,   //precision time protocol
    pub use_fake_interface: bool, //fake just because no data
    pub num_buffers: usize,  
    pub timeout_ms: u64, // debugging to not time out
}

impl CameraIngestConfig{
    pub fn from_env(camera_id: CameraId) -> Self{
        let prefix = match camera_id {
            CameraId::FieldLeft => "FIELD_LEFT", 
            CameraId::FieldRight => "FIELD_RIGHT",
        }; 

        Self{
            device_id: read_string_env(&format!("ARAVIS_CAMERA_ID_{prefix}")),
            exposure_time_us: read_f64_env(&format!("ARAVIS_EXPOSURE_US_{prefix}")),
            frame_rate_hz: read_f64_env(&format!("ARAVIS_FRAME_RATE_HZ_{prefix}")),
            aperture: read_f64_env(&format!("ARAVIS_APERTURE_{prefix}")),
            enable_ptp: read_bool_env(&format!("ARAVIS_ENABLE_PTP_{prefix}"), false),
            use_fake_interface: read_bool_env("ARAVIS_USE_FAKE_INTERFACE", false),
            num_buffers: read_usize_env("ARAVIS_NUM_BUFFERS", DEFAULT_NUM_BUFFERS),
            timeout_ms: read_u64_env("ARAVIS_TIMEOUT_MS", DEFAULT_TIMEOUT_MS),
        }
    }
}



// Ingests frames from the cameras using the GigEVision API, and enqueues
// the frames into the camera ingest sender for the camera's pipeline to
// begin processing.
pub fn ingest_frames(tx: crossbeam::channel::Sender<Frame>){
    //assigns to camera slot
    let slot = CameraSlot::next();
    let camera_id = slot.camera_id();
    let _config = CameraIngestConfig::from_env(camera_id);
    // TODO(#3): Implement Camera Ingest with Aravis.

    //rough workflow
    //1. Open the camera or by slot.
    //2. Apply settings from config 
    //3. Allocate and queue buffers
    //4. Convert each buffer to Frame
    //5. Frame to hevc through channel
    //6. 

    loop {
        let data = vec![1, 2, 3, 4];
        let context = Context::new(slot.tag());
        println!("Ingesting frame with metadata: {:?}", context);  // TODO: remove
        let _ = tx.send(Frame::new(data, context));
        thread::sleep(Duration::from_millis(3000));
    }
}


//functions to help read variables
//read camera_string
fn read_string_env(key: &str) -> Option<String> {
    env::var(key).ok().filter(|s| !s.trim().is_empty())
}

//reads env variable and parses to f64
fn read_f64_env(key: &str) -> Option<f64> {
    env::var(key).ok()?.trim().parse::<f64>().ok()
}

//parses as bool
fn read_bool_env(key: &str, default: bool) -> bool {
    match env::var(key) {
        Ok(value) => match value.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => true,
            "0" | "false" | "no" | "off" => false,
            _ => default,
        },
        Err(_) => default,
    }
}

//parses as usize
fn read_usize_env(key: &str, default: usize) -> usize {
    env::var(key)
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .unwrap_or(default)
}

//parses as u64
fn read_u64_env(key: &str, default: u64) -> u64 {
    env::var(key)
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .unwrap_or(default)
}
