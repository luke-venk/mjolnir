/// Shared code for interacting with Aravis library, used by both
/// discovery and recording tools.
use std::slice;

use crate::camera::record::CameraIngestConfig;
use aravis::prelude::*;
use aravis::{Aravis, Buffer, Camera, Stream};

/// Retrieves token to access global state of the Aravis library.
pub fn initialize_aravis() -> Aravis {
    Aravis::initialize().expect("Failed to initialize Aravis.")
}

/// Create and return a Camera object.
pub fn create_camera(camera_id: &str) -> Camera {
    // Update device list first, same as Python's Aravis.update_device_list() does.
    unsafe { aravis::get_device_list() };

    Camera::new(Some(camera_id))
        .unwrap_or_else(|_| panic!("ERROR: Failed to create camera with camera_id={camera_id}"))
}

/// Loads our information from our custom camera configuration type
/// into Aravis camera.
pub fn configure_camera(camera: &Camera, config: &CameraIngestConfig) {
    // Exposure time.
    camera
        .set_exposure_time(config.exposure_time_us)
        .expect("Failed to set exposure time in camera configuration.");

    // Frame rate.
    camera
        .set_frame_rate(config.frame_rate_hz)
        .expect("Failed to set frame rate in camera configuration.");

    // Resolution.
    let (width, height) = config.resolution.dimensions();
    camera
        .set_region(0, 0, width, height)
        .expect("Failed to set resolution in camera configuration.");

    // Aperture.
    // if let Some(aperture) = config.aperture {
    //     camera.set_float("Iris", aperture).expect("Failed to set iris to aperture value");
    // }

    // PTP enabling.
    if config.enable_ptp {
        camera
            .set_boolean("PtpEnable", true)
            .expect("Failed to enable PTP in camera configuration.");
    }

    // Packet size negotiation.
    // GigE Vision streams camera data over UDP. Automatically setting the packet
    // size works by sending test packets of decreasing sizes until one gets through
    // without fragmentation, discovering the Maximum Transmission Unit (MTU), the
    // largest payload that can travel end-to-end without being split.
    // Standard Ethernet MTU is about 1500 bytes.

    camera
        .gv_auto_packet_size()
        .expect("Failed to automatically set the packet size in camera configuration");

    println!(
        "Negotiated packet size: {}",
        camera
            .gv_get_packet_size()
            .expect("Failed to read packet size.")
    );

    camera
        .gv_set_packet_size(8064)
        .expect("Failed to manually set the packet size in camera configuration.");

    println!(
        "Negotiated packet size: {}",
        camera
            .gv_get_packet_size()
            .expect("Failed to read packet size.")
    );
    // // Packet delay.
    // camera
    //     .gv_set_packet_delay(0)
    //     .expect("Failed to set the packet delay in camera configuration.");

    // // Pixel format.
    camera
        .set_pixel_format(aravis::PixelFormat::MONO_8)
        .expect("Failed to set the pixel format in camera configuration.");

    // // Camera gains.
    // camera.set_gain(0.0).expect("Failed to set the gains in camera configuration.");

    // Frame rate enable.
    // camera.set_frame_rate_enable(true).expect("Failed to enable frame rate in camera configuration.");
}

/// Creates Aravis camera stream and allocates frame buffers.
pub fn create_stream_and_allocate_buffers(camera: &Camera, num_buffers: usize) -> Stream {
    // Opens channel between our app and the camera to handle streaming.
    let stream = camera
        .create_stream()
        .expect("Failed to create camera stream.");

    // Asks the camera how many bytes each frame will be.
    let payload_size = camera
        .payload()
        .expect("Failed to get camera payload size.");

    // Pre-allocates `num_buffers` empty buffers of exactly the payload size
    // and gives them to the stream object. This is a producer-consumer queue
    // that allows the camera to keep producing (filling the next buffer) while
    // we consume the previous buffer, preventing dropped frames.
    for _ in 0..num_buffers {
        let buffer = Buffer::new_allocate(payload_size as usize);
        stream.push_buffer(buffer);
    }

    stream
}

/// Converts an Aravis buffer into a vector of bytes.
pub fn copy_buffer_bytes(buffer: &Buffer) -> Vec<u8> {
    let (ptr, len) = buffer.data();

    if ptr.is_null() || len == 0 {
        return Vec::new();
    }

    // `ptr` is non-null and Aravis guarantees the buffer data is valid
    // for `len` bytes for the lifetime of the buffer reference.
    unsafe { slice::from_raw_parts(ptr as *const u8, len).to_vec() }
}
