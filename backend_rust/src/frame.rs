/**
 * Each camera frame will consist of metadata and the actual image.
 * 
 * TODO
 */
use std::time::SystemTime;

use crate::camera::CameraId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Frame {
    camera_id: CameraId,
    frame_id: u32,
    timestamp: SystemTime,
}

impl Frame {
    pub fn new(camera_id: CameraId, frame_id: u32, timestamp: SystemTime) -> Self {
        Self {
            camera_id,
            frame_id,
            timestamp,
        }
    }

    pub fn camera_id(&self) -> CameraId {
        self.camera_id
    }

    pub fn frame_id(&self) -> u32 {
        self.frame_id
    }

    pub fn timestamp(&self) -> SystemTime {
        self.timestamp
    }
}