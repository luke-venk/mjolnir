use std::slice;
use std::sync::Once;
use crate::schemas::{Frame, Context};
use aravis::prelude::*;
use aravis::{AcquisitionMode, Aravis, Buffer, Camera, ExposureMode};
use crate::schemas::camera_ingest_config::CameraIngestConfig;

//Intializes Aravis once
static ARAVIS_INIT: Once = Once::new();


//Intializes Aravis
pub fn initialize_aravis(){
    ARAVIS_INIT.call_once(||{
        Aravis::initialize().unwrap();
    });
}

//opens camera
pub fn open_camera(config: &CameraIngestConfig) -> Camera {
    Camera::new(config.device_id.as_deref()).unwrap()
}

//configuration values from CameraIngestConfig
pub fn configure_camera(camera: &Camera, config: &CameraIngestConfig){
    camera.set_acquisition_mode(AcquisitionMode::Continuous).unwrap();

    camera.set_exposure_mode(ExposureMode::Timed).unwrap();
    camera.set_exposure_time(config.exposure_time_us).unwrap();

    camera.set_frame_rate_enable(true).unwrap();
    camera.set_frame_rate(config.frame_rate_hz).unwrap();

    if config.enable_ptp{
        camera.set_boolean("PtpEnable", true).unwrap();
    }
}

pub fn create_stream_and_queue_buffers(camera: &Camera, num_buffers: usize) -> aravis::Stream {
    let stream = camera.create_stream().unwrap();

    let payload_size = camera.payload().unwrap() as usize;

    for _ in 0..num_buffers {
        stream.push_buffer(Buffer::new_allocate(payload_size));
    }

    stream
}

// Takes aravis camera buffer and copies it into Rust memory
//Takes raw image bytes from buffer to be stored inside pipeline Frame
pub fn copy_buffer_bytes(buffer: &Buffer) -> Vec<u8> {
    let (ptr, len) = buffer.data();

    if ptr.is_null() || len == 0 {
        return Vec::new();
    }

    unsafe { slice::from_raw_parts(ptr as *const u8, len).to_vec() }
}

//Takes aravis buffer into your frame type
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
    //build and return the pipeline Frame with data
    Frame::new(data, Context::new(timestamp))
}
