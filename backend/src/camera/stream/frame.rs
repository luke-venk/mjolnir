/// Data for a single captured frame passed from capture thread
/// to UI thread.
use std::time::Instant;

pub struct FrameData {
    // Raw Mono8 pixel bytes.
    pub pixels: Vec<u8>,
    // Frame width in pixels.
    pub width: u32,
    // Frame height in pixels.
    pub height: u32,
    // Time when frame was received.
    pub received_at: Instant,
}
