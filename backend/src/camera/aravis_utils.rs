use super::{BarrierResult, CancelableBarrier};
use crate::camera::CameraIngestConfig;
use crate::timing::global_time;
use aravis::glib::translate::ToGlibPtr;
use aravis::prelude::*;
use aravis::{Aravis, Buffer, Camera, Stream};
use aravis_sys::{arv_camera_get_string, arv_camera_get_integer};
use glib::translate::*; // To convert high-level types to raw pointers
use std::ffi::CString;
use std::ptr;
use std::slice;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::sleep;
use std::time::Duration;

/// Shared code for interacting with Aravis library, used by both
/// discovery and recording tools.

/// Retrieves token to access global state of the Aravis library.
pub fn initialize_aravis() -> Aravis {
    Aravis::initialize().expect("Failed to initialize Aravis.")
}

/// Create and return a Camera object.
pub fn create_camera(camera_id: &str) -> Result<Camera, String> {
    Camera::new(Some(camera_id))
        .map_err(|_| format!("ERROR: Failed to create camera with camera_id = {camera_id}. Please try recording/streaming again..."))
}

fn unsafe_read_camera_string(camera: &Camera, node_name: &str) -> String {
    unsafe {
        let mut error: *mut glib::ffi::GError = ptr::null_mut();
        let camera_ptr: *mut aravis_sys::ArvCamera = camera.to_glib_none().0;
        let feature_c_str = CString::new(node_name).unwrap();
        let raw_res = arv_camera_get_string(camera_ptr, feature_c_str.as_ptr(), &mut error);
        if !error.is_null() {
            panic!(
                "Error calling arv_camera_get_string for node: {}",
                node_name
            );
        }
        return from_glib_none(raw_res);
    }
}

fn unsafe_read_camera_boolean(camera: &Camera, node_name: &str) -> bool {
    unsafe {
        let mut error: *mut glib::ffi::GError = std::ptr::null_mut();
        let camera_ptr: *mut aravis_sys::ArvCamera = camera.to_glib_none().0;
        let feature_c_str = CString::new(node_name).unwrap();
        let raw_res =
            aravis_sys::arv_camera_get_boolean(camera_ptr, feature_c_str.as_ptr(), &mut error);
        if !error.is_null() {
            panic!(
                "Error calling arv_camera_get_boolean for node: {}",
                node_name
            );
        }
        raw_res != 0
    }
}

fn unsafe_read_camera_integer(camera: &Camera, node_name: &str) -> i64 {
    unsafe {
        let mut error: *mut glib::ffi::GError = ptr::null_mut();
        let camera_ptr: *mut aravis_sys::ArvCamera = camera.to_glib_none().0;
        let feature_c_str = CString::new(node_name).unwrap();
        let raw_res = arv_camera_get_integer(camera_ptr, feature_c_str.as_ptr(), &mut error);
        if !error.is_null() {
            panic!(
                "Error calling arv_camera_get_integer for node: {}",
                node_name
            );
        }
        raw_res
    }
}

fn read_ptp_time_ns(camera: &Camera) -> u64 {
    camera
        .execute_command("PtpDataSetLatch")
        .expect("Failed to latch PTP dataset.");
    unsafe_read_camera_integer(camera, "PtpDataSetLatchValue") as u64
}

/// This mutates the global time to include a PTP offset
/// It makes 50 'what PTP time is it' calls to the camera
/// Estimates the round trip time and offset
/// Removes outliers
/// And sets the PTP offset in global time to the average of the offsets after outliers are removed.
/// 'Outlier' is classified as any sample with a round-trip time (RTT) >3x the minimum one
/// Since it is hard to get good camera time if IO/OS/network jitter fucked our RTT
fn estimate_global_time_ptp_offset(camera: &Camera) {
    let mut samples = Vec::with_capacity(50);

    for _ in 0..50 {
        let gt = global_time();
        let before = gt.now_monotonic_in_nanoseconds_since_unix_epoch();
        let camera_time = read_ptp_time_ns(camera);
        let after = gt.now_monotonic_in_nanoseconds_since_unix_epoch();
        let round_trip = after - before;
        let local_midpoint = before + round_trip / 2;
        samples.push((round_trip, camera_time as i64 - local_midpoint as i64));
    }

    let min_round_trip = samples.iter().map(|(rt, _)| *rt).min().unwrap();
    let offsets: Vec<i64> = samples
        .into_iter()
        .filter(|(rt, _)| *rt < min_round_trip * 3)
        .map(|(_, offset)| offset)
        .collect();

    let avg_offset = offsets.iter().sum::<i64>() / offsets.len() as i64;
    global_time().set_approximate_additive_ptp_offset_from_wall_clock_nanoseconds(Some(avg_offset));
}

pub struct PtpConfig {
    pub is_slave: bool,
    pub enable_barrier: CancelableBarrier,
    pub configure_barrier: CancelableBarrier,
    pub lock_barrier: CancelableBarrier,
}

/// Loads our information from our custom camera configuration type
/// into Aravis camera.
pub fn configure_camera(
    camera: &Camera,
    config: &CameraIngestConfig,
    maybe_shutdown: Option<Arc<AtomicBool>>,
    maybe_ptp_config: Option<&PtpConfig>,
) {
    // Packet size.
    camera
        .gv_set_packet_size(9000)
        .expect("Failed to manually set the packet size in camera configuration.");

    // Packet delay.
    camera
        .gv_set_packet_delay(1000)
        .expect("Failed to set the packet delay in camera configuration.");

    // Clear any old trigger state
    camera
        .clear_triggers()
        .expect("Failed to clear old triggers");

    // Disable auto settings
    camera
        .set_exposure_time_auto(aravis::Auto::Off)
        .expect("Failed to disable auto exposure time.");
    camera
        .set_gain_auto(aravis::Auto::Off)
        .expect("Failed to disable auto gains.");

    // Exposure time.
    camera
        .set_exposure_time(config.exposure_time_us)
        .expect("Failed to set exposure time in camera configuration.");

    // Camera gains.
    camera
        .set_gain(0.0)
        .expect("Failed to set the gains in camera configuration.");

    // Frame rate enable.
    camera
        .set_frame_rate_enable(true)
        .expect("Failed to enable frame rate in camera configuration.");

    // Frame rate.
    camera
        .set_frame_rate(config.frame_rate_hz)
        .expect("Failed to set frame rate in camera configuration.");

    // Use binning to downsample from full resolution to lower resolution.
    if camera
        .is_binning_available()
        .expect("Error: Binning is not available for this camera.")
    {
        // Set the binning modes to "Average" instead of the default, "Sum". This
        // will prevent brightness inconsistencies between resolutions.
        // See issue #40 for more information.
        camera
            .set_string("BinningHorizontalMode", "Average")
            .expect("Failed to set BinningHorizontalMode");
        camera
            .set_string("BinningVerticalMode", "Average")
            .expect("Failed to set BinningVerticalMode");

        camera
            .set_binning(config.resolution.binning(), config.resolution.binning())
            .expect("Error: Failed to set binning for camera.");
    }

    // Pixel format.
    camera
        .set_pixel_format(aravis::PixelFormat::MONO_8)
        .expect("Failed to set the pixel format in camera configuration.");

    // PTP enabling.
    // https://support.thinklucid.com/app-note-multi-camera-synchronization-using-ptp-and-scheduled-action-commands/
    if let Some(ptp_config) = maybe_ptp_config {
        camera
            .set_boolean("PtpEnable", true)
            .expect("Failed to enable PTP in camera configuration.");
        let ptp_is_enabled = unsafe_read_camera_boolean(camera, "PtpEnable");
        if !ptp_is_enabled {
            panic!("Failed to enable ptp on camera {}", config.camera_id);
        } else {
            println!("Enabled PTP on camera {}", config.camera_id);
        }
        if ptp_config.enable_barrier.wait() == BarrierResult::Canceled {
            return;
        }
        if ptp_config.is_slave {
            camera
                .set_boolean("PtpSlaveOnly", true)
                .expect("Failed to make camera ptp slave");
            let ptp_is_slave = unsafe_read_camera_boolean(camera, "PtpSlaveOnly");
            if !ptp_is_slave {
                panic!("Failed to make camera {} a PTP slave", config.camera_id);
            } else {
                println!("Made camera {} a PTP slave", config.camera_id);
            }
            let mut success = false;
            let mut attempts = 0;
            while !success && attempts < 300 {
                if let Some(ref shutdown) = maybe_shutdown
                    && shutdown.load(Ordering::SeqCst)
                {
                    println!("Exiting configuration for camera {}.", config.camera_id);
                    return;
                }
                let ptp_status = unsafe_read_camera_string(camera, "PtpStatus");
                println!(
                    "Camera {} reads PtpStatus: {}",
                    config.camera_id, ptp_status
                );
                success = ptp_status == "Slave";
                attempts += 1;
                sleep(Duration::from_millis(500));
            }
            if !success {
                panic!("Camera {} failed to enable PTP slave", config.camera_id);
            }
        } else {
            camera
                .set_boolean("PtpSlaveOnly", false)
                .expect("Failed to make camera ptp master");
            let ptp_is_slave = unsafe_read_camera_boolean(camera, "PtpSlaveOnly");
            if !ptp_is_slave {
                println!("Made camera {} a PTP master", config.camera_id);
            } else {
                panic!("Failed to make camera {} a PTP master", config.camera_id);
            }
            let mut success = false;
            let mut attempts = 0;
            while !success && attempts < 300 {
                if let Some(ref shutdown) = maybe_shutdown
                    && shutdown.load(Ordering::SeqCst)
                {
                    println!("Exiting configuration for camera {}.", config.camera_id);
                    return;
                }
                let feature_name = "PtpStatus";
                let ptp_status = unsafe_read_camera_string(camera, feature_name);
                success = ptp_status == "Master";
                attempts += 1;
                sleep(Duration::from_millis(500));
            }
            if !success {
                panic!("Camera {} failed to enable PTP master", config.camera_id);
            }
        };
        if ptp_config.configure_barrier.wait() == BarrierResult::Canceled {
            return;
        }

        let target_status = if ptp_config.is_slave {
            "Locked"
        } else {
            "Disabled"
        };
        let mut consecutive_successes = 0;
        let mut attempts = 0;
        while consecutive_successes < 10 && attempts < 300 {
            if let Some(ref shutdown) = maybe_shutdown
                && shutdown.load(Ordering::SeqCst)
            {
                println!("Exiting configuration for camera {}.", config.camera_id);
                return;
            }
            let ptp_enabled = unsafe_read_camera_boolean(camera, "PtpEnable");
            let ptp_master_slave_status = unsafe_read_camera_string(camera, "PtpStatus");
            let ptp_servo_status = unsafe_read_camera_string(camera, "PtpServoStatus");
            println!(
                "Camera {} reads PtpServoStatus: {} | PtpEnable: {} | PtpStatus: {}",
                config.camera_id, ptp_servo_status, ptp_enabled, ptp_master_slave_status
            );
            if ptp_servo_status == target_status {
                consecutive_successes += 1;
            } else {
                consecutive_successes = 0;
            }
            attempts += 1;
            sleep(Duration::from_millis(500));
        }
        if consecutive_successes < 10 {
            panic!("Camera {} failed to establish PTP lock", config.camera_id);
        }
        if !ptp_config.is_slave {
            println!("Finding global time to camera PTP time offset...");
            estimate_global_time_ptp_offset(camera);
            println!("PTP offset calculated!");
        }
        if ptp_config.lock_barrier.wait() == BarrierResult::Canceled {
            return;
        }
    }
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
        panic!("ERROR: Aravis buffer was empty");
    }

    // `ptr` is non-null and Aravis guarantees the buffer data is valid
    // for `len` bytes for the lifetime of the buffer reference.
    unsafe { slice::from_raw_parts(ptr as *const u8, len).to_vec() }
}
