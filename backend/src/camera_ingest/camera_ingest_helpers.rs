use std::slice;
use std::sync::Once;

use aravis::prelude::*;
use aravis::{Aravis, Buffer, Camera, PixelFormat, Stream};

use crate::schemas::camera_ingest_config::CameraIngestConfig;
use crate::schemas::{Context, Frame};

static ARAVIS_INIT: Once = Once::new();

pub fn initialize_aravis() {
    ARAVIS_INIT.call_once(|| {
        Aravis::initialize().expect("Failed to initialize Aravis.");
    });
}

pub fn open_camera(config: &CameraIngestConfig) -> Camera {
    Camera::new(Some(&config.device_id))
        .unwrap_or_else(|_| panic!("Failed to open camera {}", config.device_id))
}

pub fn configure_camera(camera: &Camera, config: &CameraIngestConfig) {
    camera
        .set_exposure_time(config.exposure_time_us)
        .expect("Failed to set exposure time");

    camera
        .set_frame_rate_enable(true)
        .expect("Failed to enable frame rate");

    camera
        .set_frame_rate(config.frame_rate_hz)
        .expect("Failed to set frame rate");

    if camera
        .is_binning_available()
        .expect("Error: Binning is not available for this camera.")
    {
        camera
            .set_binning(config.resolution.binning(), config.resolution.binning())
            .expect("Error: Failed to set binning for camera.");
    }

    if let Some(aperture) = config.aperture {
        let _ = camera.set_float("Iris", aperture);
    }

    if config.enable_ptp {
        let _ = camera.set_boolean("PtpEnable", true);
    }

    camera
        .gv_set_packet_size(8064)
        .expect("Failed to set packet size");

    camera
        .gv_set_packet_delay(5000)
        .expect("Failed to set packet delay");

    camera
        .set_pixel_format(PixelFormat::MONO_8)
        .expect("Failed to set pixel format");

    camera.set_gain(0.0).expect("Failed to set gain");
}

pub fn create_stream_and_queue_buffers(camera: &Camera, num_buffers: usize) -> Stream {
    let stream = camera
        .create_stream()
        .expect("Failed to create camera stream");

    let payload = camera
        .payload()
        .expect("Failed to get camera payload size");

    for _ in 0..num_buffers {
        let buffer = Buffer::new_allocate(payload as usize);
        stream.push_buffer(buffer);
    }

    stream
}

pub fn copy_buffer_bytes(buffer: &Buffer) -> Vec<u8> {
    let (ptr, len) = buffer.data();

    if ptr.is_null() || len == 0 {
        return Vec::new();
    }

    unsafe { slice::from_raw_parts(ptr as *const u8, len).to_vec() }
}

pub fn buffer_to_frame(buffer: &Buffer) -> Frame {
    let data = copy_buffer_bytes(buffer);

    let timestamp = {
        let system_ts = buffer.system_timestamp();
        if system_ts != 0 {
            system_ts
        } else {
            let camera_ts = buffer.timestamp();
            if camera_ts != 0 {
                camera_ts
            } else {
                buffer.frame_id()
            }
        }
    };

    Frame::new(data, Context::new(timestamp))
}
